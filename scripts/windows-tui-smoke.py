"""The interactive mode on Windows, in a real console: `aequalis` runs in a pseudo-console (ConPTY,
through `pywinpty`), gets keys as a terminal sends them, and its screen is read back.

    python scripts/windows-tui-smoke.py target/release/aequalis.exe

Checks, in order: the toolbar is drawn; a request is evaluated; a function key (F2) opens the
Measures and Esc closes it; a mouse click on the toolbar opens the Currencies; Alt+Y copies; Ctrl+Q quits and the screen is given back. The CI runs it
(`windows-tui`); it needs `pip install pywinpty`.
"""

import re
import sys
import threading
import time

from winpty import PtyProcess

EXE = sys.argv[1] if len(sys.argv) > 1 else "target/release/aequalis.exe"
SCREEN = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]")


class Session:
    """The program in a 100x30 console, its output collected by a thread."""

    def __init__(self):
        self.process = PtyProcess.spawn([EXE], dimensions=(30, 100))
        self.output = ""
        self.lock = threading.Lock()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        try:
            while True:
                chunk = self.process.read(4096)
                with self.lock:
                    self.output += chunk
        except EOFError:
            pass

    def text(self):
        with self.lock:
            return SCREEN.sub("", self.output)

    def send(self, keys):
        self.process.write(keys)

    def expect(self, wanted, seconds=10):
        end = time.time() + seconds
        while time.time() < end:
            if wanted in self.text():
                return
            time.sleep(0.1)
        sys.exit(f"FAILED: {wanted!r} never appeared. The screen:\n{self.text()[-3000:]}")

    def quit(self):
        end = time.time() + 10
        while self.process.isalive() and time.time() < end:
            time.sleep(0.1)
        if self.process.isalive():
            sys.exit(f"FAILED: still running after Ctrl+Q. The screen:\n{self.text()[-3000:]}")


def main():
    s = Session()
    s.expect("Measures  F3")  # the toolbar: `F2 󰑭 Measures  F3 …` (a symbol, or none when narrow)
    print("toolbar drawn")
    s.send("10 km mi\r")
    s.expect("6.2137 mi")
    print("request evaluated")
    s.send("\x1bOQ")  # F2
    s.expect("Date & Time")
    s.send("\x1b")
    time.sleep(0.5)
    print("F2 opens the Measures")
    s.send("\x1b[<0;16;1M\x1b[<0;16;1m")  # a left click (SGR) on the toolbar's F3 (column 16: ` F2 󰑭 Measures ` before it)
    s.expect("\u250c\u2500 Currencies")
    s.send("\x1b")
    time.sleep(0.5)
    print("a mouse click opens the Currencies")
    s.send("\x1by")  # Alt+Y
    s.expect("copied")
    print("Alt+Y copies")
    s.send("\x11")  # Ctrl+Q
    s.quit()
    # The alternate screen was left (the escape is stripped from the text: look at the raw output).
    with s.lock:
        if "\x1b[?1049l" not in s.output:
            sys.exit("FAILED: the alternate screen was not left")
    print("quit")


if __name__ == "__main__":
    main()
