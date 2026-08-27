"""Serve ../ui on 127.0.0.1:1420 for tauri dev. Path is anchored to this file."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "ui"
PORT = 1420


class Handler(SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, fmt, *args):
        print("[ui]", fmt % args)


def main():
    if not (ROOT / "index.html").is_file():
        raise SystemExit(f"UI not found at {ROOT}")
    httpd = ThreadingHTTPServer(
        ("127.0.0.1", PORT),
        partial(Handler, directory=str(ROOT)),
    )
    print(f"Serving {ROOT} at http://127.0.0.1:{PORT}")
    httpd.serve_forever()


if __name__ == "__main__":
    main()
