"""Stateful virtual endpoints run through the same Rust engine as USB devices."""
import struct

import pytest
from fujimod import usb
from fujimod.usb import blocking, tracing

testing=pytest.importorskip('fujimod.usb.testing')
LIMITS=usb.FirmwareLimits(memory_read=256,memory_write=256,configuration=256,console=256)


class Peer:
    def __init__(self):
        self.calls=[]
        self.pending=b''
        self.fail_write=None
        self.wrong_result=None

    def control(self,data):
        assert type(data) is bytes
        self.calls.append(('control',data))
        kind,unused,offset,size,token=struct.unpack('>BBIIH',data)
        assert unused==0
        if kind==0xc0:
            assert token==0 and size==8
            self.pending=b'\x34\x12\0\0\0\0\x01\0'
        else:
            assert token==0x1234
            if kind in (0xc2,0xc4): self.pending=b'\x34\x12'
            elif kind==0xe1: self.pending=bytes((offset+i)%256 for i in range(size))
            else: assert kind in (0xd0,0xd1)

    def write(self,data):
        assert type(data) is bytes
        self.calls.append(('write',data))
        if self.fail_write is not None: raise self.fail_write
        if self.wrong_result is not None: return self.wrong_result
        command,_,_,_,function,_,_,address,size=struct.unpack('<9I',data)
        assert command==0x200001 and function==2
        self.pending=data+bytes((address+i)%256 for i in range(size))

    def read(self,maximum):
        self.calls.append(('read',maximum))
        data,self.pending=self.pending[:maximum],self.pending[maximum:]
        return data


def session(peer,**kwargs):
    return testing.peer_session(peer,LIMITS,ep_in=0x82,ep_out=1,**kwargs)


def test_virtual_peer_uses_production_memory_and_trace_with_selected_endpoints():
    peer=Peer()
    trace=tracing.Trace(64,4096)
    native=session(peer,trace=trace)
    assert blocking.memory.read_range(native,0x1000,260)==bytes(range(256))+bytes(range(4))
    assert native.ping()==b'\x34\x12'
    assert native.close()==b'\x34\x12'
    assert native.state==usb.SessionState.Closed
    batch=trace.drain()
    assert batch.dropped_events==0
    assert {e.endpoint for e in batch.events}=={0,0x82,1}
    assert all(e.phase in (tracing.Phase.Submitted,tracing.Phase.Completed) for e in batch.events)
    assert [struct.unpack_from('<I',data,32)[0] for kind,data in peer.calls if kind=='write']==[256,4]


@pytest.mark.parametrize('error',[RuntimeError('virtual scheduler stopped'),KeyboardInterrupt('virtual interrupt')])
def test_callback_exception_identity_is_preserved_and_session_is_fenced(error):
    peer=Peer();native=session(peer)
    peer.fail_write=error
    with pytest.raises(type(error)) as failed: native.read_memory(4,4)
    assert failed.value is error
    assert native.state==usb.SessionState.CompletionUnknown
    calls=len(peer.calls)
    with pytest.raises(usb.CompletionUnknownError): native.close()
    with pytest.raises(usb.CompletionUnknownError): native.ping()
    assert len(peer.calls)==calls


@pytest.mark.parametrize('value',[0,1,True,b'not a completion'])
def test_control_and_write_callbacks_cannot_return_magic_integers(value):
    class BadControl(Peer):
        def control(self,data): return value
    with pytest.raises(TypeError,match='return None'): session(BadControl())
    peer=Peer();native=session(peer);peer.wrong_result=value
    with pytest.raises(TypeError,match='return None'): native.read_memory(4,4)
    assert native.state==usb.SessionState.CompletionUnknown


def test_read_callback_requires_bytes_and_invalid_reply_uses_core_validation():
    class WrongType(Peer):
        def read(self,maximum): return bytearray(super().read(maximum))
    with pytest.raises(TypeError): session(WrongType())
    peer=Peer();native=session(peer)
    peer.control=lambda _: None
    peer.pending=b'x'
    with pytest.raises(usb.ProtocolError): native.ping()
    assert native.state==usb.SessionState.CompletionUnknown


@pytest.mark.parametrize('options',[{'ep_in':1,'ep_out':2},{'ep_in':0x81,'ep_out':0x82},
                                    {'ep_in':0x81,'ep_out':2,'max_reply_bytes':0}])
def test_invalid_endpoint_or_budget_never_calls_the_peer(options):
    peer=Peer()
    with pytest.raises(ValueError): testing.peer_session(peer,LIMITS,**options)
    assert peer.calls==[]
