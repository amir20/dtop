#!/usr/bin/env python3
"""Runs inside the recorded terminal: plays the demo's keystrokes against dtop.

Started by record-demo.sh. Waits for the file named by argv[1], so the capture is
already rolling, then types `dtop` and sends the timed keys below. Terminal input
is forwarded too, because dtop's startup probe reads the terminal's replies from it.
"""

import fcntl
import os
import pty
import select
import sys
import termios
import time
import tty

DOCKER = "/usr/local/bin/docker"
COMMAND = [
    DOCKER, "run", "--rm", "-it",
    # A neutral hostname for the header, and the VM's real interfaces for its network rates
    "--hostname", "demo", "--network", "host",
    "-v", "/var/run/docker.sock:/var/run/docker.sock",
    "dtop-demo", "--filter", "label=dtop.demo",
]  # fmt: skip

RIGHT, LEFT, ESC, ENTER, SPACE = "\x1b[C", "\x1b[D", "\x1b", "\r", " "

# (seconds to wait first, keys to send)
SCRIPT = [
    (7.0, "/"),
    (0.15, "a"),
    (0.15, "p"),
    (0.15, "i"),
    (0.6, ENTER),
    (2.0, RIGHT),
    (4.0, LEFT),
    (1.0, "/"),
    (0.3, ESC),
    (2.0, SPACE),
    (0.4, SPACE),
    (0.4, SPACE),
    (0.8, ENTER),
    (2.5, ESC),
    (0.5, ESC),
    (2.5, "q"),
]


def main() -> None:
    while not os.path.exists(sys.argv[1]):
        time.sleep(0.02)

    sys.stdout.write("\x1b[2J\x1b[H\x1b[2m>\x1b[0m ")
    sys.stdout.flush()
    time.sleep(0.5)
    for ch in "dtop":
        sys.stdout.write(ch)
        sys.stdout.flush()
        time.sleep(0.12)
    time.sleep(0.5)
    sys.stdout.write("\r\n")
    sys.stdout.flush()

    size = fcntl.ioctl(0, termios.TIOCGWINSZ, b"\0" * 8)
    pid, fd = pty.fork()
    if pid == 0:
        os.execv(DOCKER, COMMAND)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, size)

    saved = termios.tcgetattr(0)
    tty.setraw(0)
    try:
        steps = iter(SCRIPT)
        wait, keys = next(steps)
        due = time.monotonic() + wait
        while True:
            timeout = None if keys is None else max(0.0, due - time.monotonic())
            ready, _, _ = select.select([0, fd], [], [], timeout)
            if fd in ready:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    break
                if not data:
                    break
                os.write(1, data)
            if 0 in ready:
                os.write(fd, os.read(0, 4096))
            if keys is not None and time.monotonic() >= due:
                os.write(fd, keys.encode())
                wait, keys = next(steps, (0.0, None))
                due = time.monotonic() + wait
    finally:
        termios.tcsetattr(0, termios.TCSADRAIN, saved)
    os.waitpid(pid, 0)
    with open(sys.argv[1] + ".done", "w"):
        pass


if __name__ == "__main__":
    main()
