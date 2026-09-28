#!/usr/bin/env python3
# .devcontainer/virtual-pointer.py -- the pointer on the headless Wayland session's seat.
#
#   virtual-pointer.py              hold a pointer open until the compositor goes away
#   virtual-pointer.py wheel <n>    scroll <n> wheel notches at the cursor; positive is down
#
# The seat has no hardware, so without this it has no pointer at all, and an app on it never binds
# one: sway's own `seat seat0 cursor set <x> <y>` and `cursor press|release button1` are accepted
# and reach nothing. sway-config starts the first form, so the pointer exists for the whole session
# and before any app connects; with it there, those two sway commands move and click.
#
# The wheel is the one thing sway's cursor commands do not deliver here -- `cursor press button5`
# scrolls nothing -- so the second form sends it through a pointer of its own, which lives only as
# long as the call. It relies on the held pointer to keep the seat's pointer capability, which is
# what the app bound when it started.
#
# The two wlroots interfaces are declared below rather than generated: pywayland bundles the core
# protocol only, and generating these would need their XML, which Ubuntu ships only inside a Rust
# crate package. Every request is declared, in protocol order, because the position of each is its
# opcode in the interface table the compositor checks against.

import sys
import time

from pywayland.client import Display
from pywayland.protocol.wayland import WlOutput, WlSeat
from pywayland.protocol_core import Argument, ArgumentType, Interface, Proxy


class ZwlrVirtualPointerV1(Interface):
    name = "zwlr_virtual_pointer_v1"
    version = 2


class ZwlrVirtualPointerV1Proxy(Proxy):
    interface = ZwlrVirtualPointerV1

    @ZwlrVirtualPointerV1.request(
        Argument(ArgumentType.Uint), Argument(ArgumentType.Fixed), Argument(ArgumentType.Fixed)
    )
    def motion(self, time, dx, dy):
        self._marshal(0, time, dx, dy)

    @ZwlrVirtualPointerV1.request(*[Argument(ArgumentType.Uint)] * 5)
    def motion_absolute(self, time, x, y, x_extent, y_extent):
        self._marshal(1, time, x, y, x_extent, y_extent)

    @ZwlrVirtualPointerV1.request(*[Argument(ArgumentType.Uint)] * 3)
    def button(self, time, button, state):
        self._marshal(2, time, button, state)

    @ZwlrVirtualPointerV1.request(
        Argument(ArgumentType.Uint), Argument(ArgumentType.Uint), Argument(ArgumentType.Fixed)
    )
    def axis(self, time, axis, value):
        self._marshal(3, time, axis, value)

    @ZwlrVirtualPointerV1.request()
    def frame(self):
        self._marshal(4)

    @ZwlrVirtualPointerV1.request(Argument(ArgumentType.Uint))
    def axis_source(self, axis_source):
        self._marshal(5, axis_source)

    @ZwlrVirtualPointerV1.request(Argument(ArgumentType.Uint), Argument(ArgumentType.Uint))
    def axis_stop(self, time, axis):
        self._marshal(6, time, axis)

    @ZwlrVirtualPointerV1.request(
        Argument(ArgumentType.Uint),
        Argument(ArgumentType.Uint),
        Argument(ArgumentType.Fixed),
        Argument(ArgumentType.Int),
    )
    def axis_discrete(self, time, axis, value, discrete):
        self._marshal(7, time, axis, value, discrete)

    @ZwlrVirtualPointerV1.request(version=1)
    def destroy(self):
        self._marshal(8)
        self._destroy()


ZwlrVirtualPointerV1._gen_c()
ZwlrVirtualPointerV1.proxy_class = ZwlrVirtualPointerV1Proxy


class ZwlrVirtualPointerManagerV1(Interface):
    name = "zwlr_virtual_pointer_manager_v1"
    version = 2


class ZwlrVirtualPointerManagerV1Proxy(Proxy):
    interface = ZwlrVirtualPointerManagerV1

    @ZwlrVirtualPointerManagerV1.request(
        Argument(ArgumentType.Object, interface=WlSeat, nullable=True),
        Argument(ArgumentType.NewId, interface=ZwlrVirtualPointerV1),
    )
    def create_virtual_pointer(self, seat):
        return self._marshal_constructor(0, ZwlrVirtualPointerV1, seat)

    @ZwlrVirtualPointerManagerV1.request(version=1)
    def destroy(self):
        self._marshal(1)
        self._destroy()

    @ZwlrVirtualPointerManagerV1.request(
        Argument(ArgumentType.Object, interface=WlSeat, nullable=True),
        Argument(ArgumentType.Object, interface=WlOutput, nullable=True),
        Argument(ArgumentType.NewId, interface=ZwlrVirtualPointerV1),
        version=2,
    )
    def create_virtual_pointer_with_output(self, seat, output):
        return self._marshal_constructor(2, ZwlrVirtualPointerV1, seat, output)


ZwlrVirtualPointerManagerV1._gen_c()
ZwlrVirtualPointerManagerV1.proxy_class = ZwlrVirtualPointerManagerV1Proxy

# From linux/input-event-codes.h and the wl_pointer enums.
AXIS_VERTICAL = 0
AXIS_SOURCE_WHEEL = 0
# One wheel notch, as libinput reports it.
NOTCH = 15.0


def connect():
    """The display, and a virtual pointer on its first seat."""
    display = Display()
    display.connect()
    found = {}

    def on_global(registry, name, interface, version):
        if interface == "wl_seat" and "seat" not in found:
            found["seat"] = registry.bind(name, WlSeat, 1)
        elif interface == ZwlrVirtualPointerManagerV1.name:
            found["manager"] = registry.bind(name, ZwlrVirtualPointerManagerV1, 1)

    registry = display.get_registry()
    registry.dispatcher["global"] = on_global
    display.roundtrip()
    missing = {"seat", "manager"} - found.keys()
    if missing:
        sys.exit(f"virtual-pointer.py: the compositor offers no {', '.join(sorted(missing))}")
    pointer = found["manager"].create_virtual_pointer(found["seat"])
    display.roundtrip()
    return display, pointer


def hold():
    display, _pointer = connect()
    # Returns -1 once the compositor closes the connection, which is when the pointer ends too.
    while display.dispatch(block=True) != -1:
        pass


def wheel(notches):
    display, pointer = connect()
    step = 1 if notches > 0 else -1
    for _ in range(abs(notches)):
        now = int(time.monotonic() * 1000) & 0xFFFFFFFF
        pointer.axis_source(AXIS_SOURCE_WHEEL)
        pointer.axis_discrete(now, AXIS_VERTICAL, NOTCH * step, step)
        pointer.frame()
        display.flush()
        time.sleep(0.05)
    display.roundtrip()
    display.disconnect()


if __name__ == "__main__":
    match sys.argv[1:]:
        case []:
            hold()
        case ["wheel", n] if n.lstrip("-").isdigit():
            wheel(int(n))
        case _:
            sys.exit("usage: virtual-pointer.py [wheel <notches>]")
