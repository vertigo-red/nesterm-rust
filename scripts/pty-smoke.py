#!/usr/bin/env python3
"""Exercise the actual CLI in a POSIX PTY. Uses only Python's standard library."""
import errno
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

ENTER = b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H\x1b[40m\x1b[97m\x1b[>3u"
EXIT = b"\x1b[<u\x1b[0m\x1b[?25h\x1b[?1049l"


def make_rom():
    # A generated, original NROM. NMI cycles the backdrop color every frame.
    prg = bytearray([0xEA] * 16384)
    init = bytes.fromhex(
        "78 d8 a2 ff 9a e8 8e 00 20 8e 01 20 "
        "2c 02 20 10 fb "
        "a9 3f 8d 06 20 a9 00 8d 06 20 a9 21 8d 07 20 "
        "a9 80 8d 00 20 a9 0a 8d 01 20"
    )
    prg[:len(init)] = init
    main = 0x8000 + len(init)
    prg[len(init):len(init)+3] = bytes([0x4C, main & 255, main >> 8])
    nmi = bytes.fromhex(
        "48 a9 3f 8d 06 20 a9 00 8d 06 20 "
        "e6 00 a5 00 29 0f 09 10 8d 07 20 "
        "a9 00 8d 05 20 8d 05 20 68 40"
    )
    prg[0x100:0x100+len(nmi)] = nmi
    prg[0x3FFA:] = bytes.fromhex("00 81 00 80 00 80")
    return b"NES\x1a" + bytes([1, 1] + [0] * 10) + prg + bytes(8192)


class Session:
    def __init__(self, binary, args, width=64, height=30):
        self.master, self.slave = pty.openpty()
        self.resize(width, height)
        self.original = termios.tcgetattr(self.slave)
        self.output = bytearray()
        self.process = subprocess.Popen(
            [str(binary), *map(str, args)], stdin=self.slave, stdout=self.slave,
            stderr=subprocess.PIPE, start_new_session=True,
        )

    def resize(self, width, height):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))

    def pump(self, timeout=0.02):
        readable, _, _ = select.select([self.master], [], [], timeout)
        if readable:
            try:
                data = os.read(self.master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                return
            self.output.extend(data)

    def wait_output(self, sequence, timeout=5):
        deadline = time.monotonic() + timeout
        while sequence not in self.output and time.monotonic() < deadline:
            self.pump()
        assert sequence in self.output, (self.process.poll(), self.output[:100])

    def send(self, data):
        os.write(self.master, data)

    def finish(self, expected=0):
        deadline = time.monotonic() + 5
        try:
            while self.process.poll() is None and time.monotonic() < deadline:
                self.pump()
            if self.process.poll() is None:
                self.process.kill()
                raise AssertionError("CLI did not terminate within 5 seconds")
            for _ in range(4):
                self.pump(0.01)
            stderr = self.process.stderr.read().decode()
            assert self.process.returncode == expected, stderr
            assert termios.tcgetattr(self.slave) == self.original, "termios settings were not restored"
            return bytes(self.output), stderr
        finally:
            if self.process.poll() is None:
                self.process.kill()
            self.process.wait()
            self.process.stderr.close()
            os.close(self.master)
            os.close(self.slave)


def main():
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/nesterm").resolve()
    with tempfile.TemporaryDirectory(prefix="nesterm-pty-") as tmp:
        tmp = Path(tmp)
        rom = tmp / "test fixture.nes"
        rom.write_bytes(make_rom())
        cast = tmp / "session.cast"
        session = Session(binary, [rom, "--size", "64x30", "--mode", "ramp", "--seconds", "0.3", "--record", cast])
        output, _ = session.finish()
        assert output.startswith(ENTER) and output.endswith(EXIT)
        events = [json.loads(line) for line in cast.read_text().splitlines()]
        assert events[0]["width"] == 64 and events[0]["height"] == 30
        assert "".join(event[2] for event in events[1:]).encode() == output
        print("PASS exact-size PTY, raw-mode restoration, and byte-exact cast")

        session = Session(binary, [tmp / "missing.nes"], width=39, height=25)
        output, error = session.finish(expected=1)
        assert not output and "terminal is too small" in error
        print("PASS undersized PTY rejected before ROM access")

        session = Session(binary, [rom, "--size", "64x30"])
        session.wait_output(ENTER)
        session.resize(63, 30)
        output, error = session.finish(expected=1)
        assert output.endswith(EXIT) and "terminal is too small" in error
        print("PASS shrinking PTY restores terminal and fails cleanly")

        for name, data in [("legacy Q", b"q"), ("Kitty Ctrl-C", b"\x1b[99;5u")]:
            session = Session(binary, [rom])
            session.wait_output(ENTER)
            # Deliver an escape sequence split over separate reads.
            session.send(b"\x1b[120;1:")
            time.sleep(0.015)
            session.send(b"1u\x1b[120;1:3u" + data)
            output, _ = session.finish()
            assert output.endswith(EXIT)
            print(f"PASS fragmented Kitty input and {name}")

        session = Session(binary, [rom])
        session.wait_output(ENTER)
        session.process.send_signal(signal.SIGTERM)
        output, _ = session.finish()
        assert output.endswith(EXIT)
        print("PASS SIGTERM restores terminal and raw mode")

        session = Session(binary, [rom, "--mode", "ramp", "--fps", "60"])
        session.wait_output(b"\x1b[39m")
        session.send(b"p")
        deadline = time.monotonic() + 0.20
        while time.monotonic() < deadline:
            session.pump()
        before = len(session.output)
        deadline = time.monotonic() + 0.15
        while time.monotonic() < deadline:
            session.pump()
        assert len(session.output) == before, "paused game kept changing the display"
        session.send(b"p")
        deadline = time.monotonic() + 0.15
        while len(session.output) == before and time.monotonic() < deadline:
            session.pump()
        assert len(session.output) > before, "resume did not advance emulation"
        session.send(b"q")
        output, _ = session.finish()
        assert output.endswith(EXIT)
        print("PASS pause freezes emulation and resume advances it")


if __name__ == "__main__":
    main()
