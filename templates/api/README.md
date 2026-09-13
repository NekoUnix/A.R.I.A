# ARIA API starter

![Current API settings](../../docs/images/api-v23.png)

Enable **Settings → Developer API**, copy the key, and set the `ARIA_API_KEY`
environment variable in your terminal. Never commit a key. Python uses only its
standard library; PowerShell uses `Invoke-RestMethod`.

```powershell
$env:ARIA_API_KEY = Read-Host 'Paste your local ARIA API key'
python ./templates/api/aria_client.py
./templates/api/control.ps1
```

The Python example displays current state, freezes for two seconds and resumes.
The PowerShell example applies Sonoma Dark. Both verify the returned command ticket.
Read [the API guide](../../docs/api.md) before writing an integration. The model
generation protects against controlling an avatar that replaced your original target.
