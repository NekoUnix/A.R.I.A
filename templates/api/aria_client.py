"""Python 3 standard-library client. Set ARIA_API_KEY and enable ARIA's API."""
import json
import os
import time
import urllib.request


class Aria:
    def __init__(self, key=None, port=39421):
        self.key = key or os.environ['ARIA_API_KEY']
        self.base = f'http://127.0.0.1:{int(port)}'

    def request(self, path, body=None):
        data = None if body is None else json.dumps(body, allow_nan=False).encode()
        request = urllib.request.Request(self.base + path, data=data,
            headers={'Authorization': f'Bearer {self.key}', 'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=3) as response:
            return json.load(response)

    def state(self):
        return self.request('/v1/state')

    def command(self, action, state=None):
        state = state or self.state()
        queued = self.request('/v1/commands',
            dict(version=1, generation=state['generation'], action=action))
        for _ in range(25):
            time.sleep(.2)
            result = self.request(f"/v1/commands/{queued['ticket']}")
            if result['status'] == 'applied':
                return result
            if result['status'] == 'rejected':
                raise RuntimeError(result['error'])
        raise TimeoutError('Command is still pending; inspect its ticket before retrying')


if __name__ == '__main__':
    aria = Aria()
    state = aria.state()
    print(json.dumps(state, indent=2))
    # Example: freeze the current avatar, then resume. Remove these lines if
    # you only want to inspect state. No assumptions about a model's parameter IDs.
    aria.command(dict(type='pose', frozen=True), state)
    try:
        time.sleep(2)
    finally:
        aria.command(dict(type='pose', frozen=False), state)
