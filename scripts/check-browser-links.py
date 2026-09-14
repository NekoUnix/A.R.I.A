"""Opt-in desktop test: a real default browser must visit a local test server.

Build `cargo build -p aria-desktop --features screenshots`, then run this script
with that executable's path. Requires an interactive desktop and default browser.
Normal release builds do not contain the smoke hook. Nothing leaves localhost.
"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import threading


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    args = parser.parse_args()
    executable = args.executable.resolve(strict=True)
    received = threading.Event()
    endpoint = "/aria-browser-check-" + secrets.token_hex(16)
    observed = []

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path != endpoint:
                self.send_error(404)
                return
            observed.append(self.headers.get("User-Agent", ""))
            body = (b"<!doctype html><title>ARIA browser link check</title>"
                    b"<h1>ARIA opened your browser successfully.</h1>"
                    b"<p>This local test sent no data to an external service. "
                    b"You can close this tab.</p>")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            received.set()

        def log_message(self, *_args):
            pass

    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory(prefix="aria-browser-test-") as temp:
                folder = Path(temp)
                # Keep this synthetic demo separate from the owner's avatars/settings.
                env = {key: value for key, value in os.environ.items()
                       if not key.startswith("ARIA_")}
                env.update(ARIA_PROFILE_DIR=temp, APPDATA=temp, LOCALAPPDATA=temp,
                           XDG_CONFIG_HOME=temp, ARIA_SMOKE_SCENARIO="browser-link",
                           ARIA_SCREENSHOT_TO=str(folder / "browser-smoke.png"),
                           ARIA_SMOKE_DELAY_SECONDS="8",
                           ARIA_SMOKE_BROWSER_URL=f"http://127.0.0.1:{server.server_port}{endpoint}")
                with (folder / "app.log").open("w", encoding="utf-8") as log:
                    process = subprocess.Popen([str(executable)], env=env,
                                               stdout=log, stderr=log)
                    try:
                        returncode = process.wait(timeout=45)
                    except subprocess.TimeoutExpired:
                        # This is exclusively the test process created above.
                        process.terminate()
                        process.wait(timeout=10)
                        raise RuntimeError("Smoke app did not close; build with screenshots")
                if returncode or not received.wait(timeout=5) or not any(observed):
                    print((folder / "app.log").read_text(encoding="utf-8", errors="replace"))
                    raise RuntimeError("No browser visit received through native OpenUrl dispatch")
                if not (folder / "browser-smoke.png").is_file():
                    raise RuntimeError("Native frame capture missing")
                print("PASS: native ARIA OpenUrl launched the default browser; localhost GET received.")
        finally:
            server.shutdown()
            thread.join(timeout=5)


if __name__ == "__main__":
    main()
