"""Opaque USB bus operations and paused observations on a native endpoint."""
import json

class EndpointPort:
    def __init__(self, runtime, node='usb'):
        self.runtime, self.node = runtime, node

    def status(self):
        return json.loads(self.runtime.usb_status(self.node))

    def reset(self):
        self.runtime.usb_reset(self.node)

    def setup(self, packet):
        self.runtime.usb_setup(self.node, packet)

    def out(self, endpoint, packet):
        self.runtime.usb_out(self.node, endpoint, packet)

    def in_packet(self, endpoint, size):
        return self.runtime.usb_in(self.node, endpoint, size)

    def in_token(self, endpoint):
        self.runtime.usb_in_token(self.node, endpoint)
