"""Prepared E0 uploads use one native session owner across every stage."""
import struct
import pytest
from fujimod import usb
from fujimod.usb import data

testing=pytest.importorskip('fujimod.usb.testing')
LIMITS=usb.FirmwareLimits(memory_read=1,memory_write=1,configuration=1,console=1)


class UploadPeer:
    def __init__(self,*,status=(0,0),failure=None,acknowledgement=b'\xff\x01\x80\x7f'):
        self.acknowledgement=acknowledgement
        self.status,self.failure=status,failure
        self.controls=[];self.pieces=[];self.setup=None;self.pending=b'';self.kind=None

    def control(self,raw):
        kind,_,offset,size,token=struct.unpack('>BBIIH',raw)
        self.controls.append(kind);self.kind=kind
        if kind==0xc0:
            assert token==0
            self.pending=b'\x34\x12\0\0\0\0\0\x01'  # debug limit1 is not an E0 limit
        else:
            assert token==0x1234
            if kind==0xe8: self.pending=b'x' if self.failure=='final-short' else self.acknowledgement
            elif kind==0xc4: self.pending=b'\x34\x12'
            else: assert kind in (0xd0,0xd1,0xe0)

    def write(self,raw):
        if self.kind==0xd0:
            fields=struct.unpack('<8I',raw)
            assert fields[:5]==(0x760001,0,0,0,0) and fields[7]==0
            self.setup=(fields[5],fields[6])
            self.pending=struct.pack('<8I',0x760001,*self.status,0,9,8,7,6)
            if self.failure=='setup-short': self.pending=b'x'
        else:
            assert self.kind==0xe0
            self.pieces.append(raw)
            if self.failure==len(self.pieces): raise OSError('virtual upload lost')

    def read(self,maximum):
        raw,self.pending=self.pending[:maximum],self.pending[maximum:]
        return raw


async def opened(peer,**options):
    return (await testing.peer_session(peer,LIMITS,ep_in=0x82,ep_out=1,**options))


async def test_upload_returns_raw_setup_and_opaque_final_bytes_without_padding():
    peer=UploadPeer();session=(await opened(peer))
    raw=bytes(range(256))+b'XYZ'
    result=(await data.upload(session,raw,piece_bytes=128,mode=usb.DataWriteMode.Preserve))
    assert peer.setup==(len(raw),0)
    assert list(map(len,peer.pieces))==[128,128,3]
    assert b''.join(peer.pieces)==raw
    assert peer.controls==[0xc0,0xd0,0xd1,0xe0,0xe0,0xe0,0xe8]
    assert result.setup.status==usb.CommandStatus.Success
    assert result.setup.fields==[9,8,7,6]
    assert result.acknowledgement==b'\xff\x01\x80\x7f'
    assert result.bytes_written==len(raw)
    assert session.state==usb.SessionState.Ready


@pytest.mark.parametrize('status',[(1,0xffff),(0,1)])
async def test_refused_setup_is_a_completed_error_without_payload_retry_or_finalization(status):
    peer=UploadPeer(status=status);session=(await opened(peer))
    with pytest.raises(usb.DeviceCommandError) as error:
        (await data.upload(session,b'abc',piece_bytes=2,mode=usb.DataWriteMode.FillOnes))
    assert (error.value.command,error.value.primary,error.value.secondary)==(0x760001,*status)
    assert peer.setup==(3,1)
    assert peer.pieces==[] and peer.controls==[0xc0,0xd0,0xd1]
    assert session.state==usb.SessionState.Ready


@pytest.mark.parametrize('failure',['setup-short',1,2,'final-short'])
async def test_failed_upload_never_retries_or_allows_cleanup(failure):
    peer=UploadPeer(failure=failure);session=(await opened(peer))
    with pytest.raises((usb.ProtocolError,OSError)):
        (await data.upload(session,b'abc',piece_bytes=2,mode=usb.DataWriteMode.Echo))
    assert peer.setup==(3,0x100)
    assert session.state==usb.SessionState.CompletionUnknown
    calls=peer.controls.copy()
    with pytest.raises(usb.CompletionUnknownError): (await session.close())
    assert peer.controls==calls
    assert peer.controls.count(0xe8)==int(failure=='final-short')


async def test_invalid_lengths_modes_and_final_budget_fail_before_setup():
    peer=UploadPeer();session=(await opened(peer,max_reply_bytes=31))
    with pytest.raises(ValueError): (await data.upload(session,b'abc',piece_bytes=2,mode=usb.DataWriteMode.Preserve))
    assert peer.setup is None and session.state==usb.SessionState.Ready
    session=(await opened(peer))
    for raw,piece,mode,exception in [(b'',2,usb.DataWriteMode.Preserve,ValueError),
                                   (b'abc',0,usb.DataWriteMode.Preserve,ValueError),
                                   (b'abc',2,0,TypeError),
                                   (b'abc',2,usb.DataReadMode.Echo,TypeError)]:
        with pytest.raises(exception): (await data.upload(session,raw,piece_bytes=piece,mode=mode))
    assert peer.setup is None and session.state==usb.SessionState.Ready


async def test_captured_x70_32_byte_e8_reply_completes_upload_and_allows_close():
    # X70 1.10, 2026-10-18T00:22:22Z: E8 32-byte receive after 64 KiB E0 pieces.
    reply=bytes.fromhex('0100760000000000000000000000000000000000000000000000000000000000')
    peer=UploadPeer(acknowledgement=reply);session=(await opened(peer))
    raw=b'X'*65536
    result=(await data.upload(session,raw,piece_bytes=65536,mode=usb.DataWriteMode.Preserve))
    assert result.acknowledgement==reply and result.bytes_written==len(raw)
    assert peer.pieces==[raw] and peer.controls==[0xc0,0xd0,0xd1,0xe0,0xe8]
    assert session.state==usb.SessionState.Ready
    (await session.close())
    assert session.state==usb.SessionState.Closed and peer.controls[-1]==0xc4


@pytest.mark.parametrize('word,value,exception,ready',[
    (0,0x550001,usb.ProtocolError,False),
    (3,1,usb.ProtocolError,False),
    (1,1,usb.DeviceCommandError,True),
    (2,0xffff,usb.DeviceCommandError,True),
])
async def test_long_e8_validates_command_reserved_and_status_without_replay(word,value,exception,ready):
    reply=bytearray(struct.pack('<8I',0x760001,0,0,0,9,8,7,6))
    struct.pack_into('<I',reply,word*4,value)
    peer=UploadPeer(acknowledgement=bytes(reply));session=(await opened(peer))
    with pytest.raises(exception): (await data.upload(session,b'abc',piece_bytes=3,mode=usb.DataWriteMode.Preserve))
    assert peer.controls==[0xc0,0xd0,0xd1,0xe0,0xe8]
    assert session.state==(usb.SessionState.Ready if ready else usb.SessionState.CompletionUnknown)
    if not ready:
        with pytest.raises(usb.CompletionUnknownError): (await session.close())
        assert peer.controls[-1]==0xe8
