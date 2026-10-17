"""Capabilities metadata is one request; reading its table is a separate action."""
import struct
import pytest
from fujimod import usb

testing=pytest.importorskip('fujimod.usb.testing')
LIMITS=usb.FirmwareLimits(memory_read=1,memory_write=1,configuration=1,console=1)


def session(reply):
    exchange=testing.Exchange
    control=lambda kind,size,token:exchange.control(struct.pack('>BBIIH',kind,0,0,size,token))
    request=struct.pack('<9I',0x580000,0,0,0,0,0x100,0,0,0)
    return testing.session([
        control(0xc0,8,0),exchange.read(8,b'\x34\x12\0\0\0\0\x01\0'),
        control(0xd0,36,0x1234),exchange.write(request),
        control(0xd1,32,0x1234),exchange.read(32,reply),
    ],LIMITS)


@pytest.mark.parametrize('length',[0,48,128,0xffffffff])
def test_metadata_retains_firmware_size_without_implicit_table_read(length):
    raw=struct.pack('<8I',0x580000,0,0,0,0,length,0x12345678,0x87654321)
    camera,script=session(raw)
    reply=camera.prepare_capabilities()
    assert reply.byte_length==length and reply.raw==raw
    assert script.remaining==0 and camera.state==usb.SessionState.Ready
    with pytest.raises(AttributeError):reply.byte_length=1


@pytest.mark.parametrize('function,exception,state',[
    (0,usb.DeviceCommandError,usb.SessionState.Ready),
    (1,usb.ProtocolError,usb.SessionState.CompletionUnknown),
])
def test_error_reply_is_complete_only_when_its_header_matches(function,exception,state):
    camera,script=session(struct.pack('<8I',0x580000,1,0xffff,0,function,48,0,0))
    with pytest.raises(exception) as error:camera.prepare_capabilities()
    assert script.remaining==0 and camera.state==state
    if state==usb.SessionState.Ready:
        assert (error.value.command,error.value.primary,error.value.secondary)==(0x580000,1,0xffff)
    else:
        with pytest.raises(usb.CompletionUnknownError):camera.close()
