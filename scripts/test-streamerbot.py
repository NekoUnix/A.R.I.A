"""Compile the actual C# connector in PowerShell 7 and exercise HTTP outcomes.

Uses a CPH adapter and loopback fixture, not a live Streamer.bot account.
Run: python scripts/test-streamerbot.py [--powershell /path/to/pwsh]
"""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import subprocess
import tempfile
import threading


class Fixture(BaseHTTPRequestHandler):
    scenario = "connected"
    posts = 0
    polls = 0

    def log_message(self, *_):
        pass

    def reply(self, code, value):
        data = json.dumps(value).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path.startswith("/_case/"):
            Fixture.scenario = self.path.rsplit("/", 1)[1]
            Fixture.posts = Fixture.polls = 0
            return self.reply(200, {})
        if self.path == "/_counts":
            return self.reply(200, {"posts": Fixture.posts})
        assert self.headers["Authorization"] == "Bearer " + "a" * 48
        if self.path == "/v1/state":
            return self.reply(200, {"version": 1, "generation": 7, "state": {}})
        assert self.path == "/v1/commands/5"
        Fixture.polls += 1
        if Fixture.scenario == "rejected":
            return self.reply(200, {"status": "rejected", "error": "Object was removed"})
        status = "queued" if Fixture.polls == 1 or Fixture.scenario == "pending" else "applied"
        return self.reply(200, {"status": status})

    def do_POST(self):
        assert self.path == "/v1/commands"
        assert self.headers["Authorization"] == "Bearer " + "a" * 48
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        assert body == {"version": 1, "generation": 7, "action": {
            "type": "workspace_action", "target": {"Avatar": {"profile": 42, "command": {"Item": 3}}}, "mode": "Off"}}
        Fixture.posts += 1
        if Fixture.scenario == "stale":
            return self.reply(409, {"error": "Model changed"})
        self.reply(202, {"ticket": 5, "queued": True})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--powershell", default="pwsh")
    args = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        with tempfile.TemporaryDirectory(prefix="aria-streamerbot-") as folder:
            results = Path(folder) / "results.json"
            subprocess.run([args.powershell, "-NoProfile", "-File",
                str(Path(__file__).with_suffix(".ps1")), "-Port", str(server.server_port),
                "-Results", str(results)], check=True, timeout=45)
            rows = json.loads(results.read_text(encoding="utf-8-sig"))
            expected = {"connected": (True, "connected", 0), "applied": (True, "applied", 1),
                "rejected": (False, "rejected", 1), "stale": (False, "error", 1),
                "pending": (False, "pending", 1), "malformed": (False, "error", 0),
                "missing_key": (False, "error", 0)}
            assert len(rows) == len(expected)
            for row in rows:
                assert (row["success"], row["status"], row["posts"]) == expected[row["scenario"]], row
                if not row["success"]:
                    assert row["error"]
                print("C# connector passed:", row["scenario"])
    finally:
        server.shutdown()
        server.server_close()
        worker.join()


if __name__ == "__main__":
    main()
