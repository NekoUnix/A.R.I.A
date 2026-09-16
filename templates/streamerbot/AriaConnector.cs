// ARIA Streamer.bot connector. Paste into Core > C# > Execute C# Code.
// Set persisted globals ariaApiKey (text) and ariaPort (number/text, default 39421).
// Optional action arguments: ariaAction (JSON), ariaWaitSeconds (1-600, default 75).
// Empty action is a read-only connection test. Never put secrets in this file.
using System;
using System.IO;
using System.Net;
using System.Text;
using System.Threading;
using System.Diagnostics;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

public class CPHInline
{
    private const string DefaultActionBase64 = "__ARIA_ACTION_BASE64__";
    private const int DefaultPort = 39421;

    public bool Execute()
    {
        CPH.SetArgument("ariaSuccess", false);
        CPH.SetArgument("ariaStatus", "error");
        CPH.SetArgument("ariaTicket", 0UL);
        CPH.SetArgument("ariaError", "");
        CPH.SetArgument("ariaState", "");
        try
        {
            string key = CPH.GetGlobalVar<string>("ariaApiKey", true);
            if (String.IsNullOrWhiteSpace(key))
                throw new InvalidOperationException("Set the persisted ariaApiKey global from ARIA's Copy API key button.");
            key = key.Trim();
            if (!System.Text.RegularExpressions.Regex.IsMatch(key, "\\A[0-9a-fA-F]{48}\\z"))
                throw new InvalidOperationException("ariaApiKey must be the 48-character key copied from ARIA.");
            object savedPort = CPH.GetGlobalVar<object>("ariaPort", true);
            int port = DefaultPort;
            if (savedPort != null && !Int32.TryParse(Convert.ToString(savedPort), out port))
                throw new InvalidOperationException("ariaPort must be a number.");
            if (port < 1024 || port > 65535) throw new InvalidOperationException("ariaPort must be between 1024 and 65535.");
            string action;
            if (!CPH.TryGetArg<string>("ariaAction", out action))
                action = Encoding.UTF8.GetString(Convert.FromBase64String(DefaultActionBase64.Replace("__ARIA_ACTION_BASE64__", "")));
            int waitSeconds;
            if (!CPH.TryGetArg<int>("ariaWaitSeconds", out waitSeconds)) waitSeconds = 75;
            if (waitSeconds < 1 || waitSeconds > 600) throw new InvalidOperationException("ariaWaitSeconds must be 1-600.");
            int status;
            JObject state = Send(port, key, "GET", "/v1/state", null, out status);
            Require(status == 200, Explain(status, state));
            CPH.SetArgument("ariaState", state.ToString(Formatting.None));
            if (String.IsNullOrWhiteSpace(action))
            {
                CPH.SetArgument("ariaStatus", "connected");
                CPH.SetArgument("ariaSuccess", true);
                CPH.LogInfo("ARIA connection test passed.");
                return true;
            }
            JObject body = new JObject {
                ["version"] = 1, ["generation"] = state["generation"],
                ["action"] = JObject.Parse(action)
            };
            string json = body.ToString(Formatting.None);
            Require(Encoding.UTF8.GetByteCount(json) <= 4096, "Action exceeds ARIA's 4 KiB request limit.");
            // POST is never retried: a timeout may mean ARIA already accepted it.
            JObject queued = Send(port, key, "POST", "/v1/commands", json, out status);
            Require(status == 202, Explain(status, queued));
            ulong ticket = queued.Value<ulong>("ticket");
            Require(ticket != 0, "ARIA returned no result ticket. Do not repeat the trigger blindly.");
            CPH.SetArgument("ariaTicket", ticket);
            Stopwatch timer = Stopwatch.StartNew();
            while (timer.Elapsed.TotalSeconds < waitSeconds)
            {
                Thread.Sleep(250);
                JObject result = Send(port, key, "GET", "/v1/commands/" + ticket, null, out status);
                if (status == 429) { Thread.Sleep(750); continue; }
                Require(status == 200, Explain(status, result));
                string outcome = result.Value<string>("status");
                if (outcome == "queued") continue;
                if (outcome == "rejected")
                {
                    CPH.SetArgument("ariaStatus", "rejected");
                    throw new InvalidOperationException(result.Value<string>("error") ?? "ARIA rejected the action.");
                }
                Require(outcome == "applied", "Unexpected ticket status. Inspect ARIA before retrying.");
                CPH.SetArgument("ariaStatus", "applied");
                CPH.SetArgument("ariaSuccess", true);
                return true;
            }
            CPH.SetArgument("ariaStatus", "pending");
            throw new InvalidOperationException("ARIA ticket " + ticket + " is still pending. It may finish later; do not repeat the trigger. Check ARIA or poll this ticket.");
        }
        catch (WebException ex)
        {
            return Fail("Connection failed (" + ex.Status + "). Check ARIA, its API switch and the port. If a command was sent it may already have run; do not blindly retry.");
        }
        catch (JsonException)
        {
            return Fail("Invalid JSON in ariaAction or ARIA's response. Copy a fresh action from ARIA setup.");
        }
        catch (Exception ex) { return Fail(ex.Message); }
    }

    private bool Fail(string message)
    {
        CPH.SetArgument("ariaError", message);
        CPH.LogError("ARIA: " + message);
        return false;
    }
    private static void Require(bool valid, string message)
    {
        if (!valid) throw new InvalidOperationException(message);
    }
    private static string Explain(int status, JObject body)
    {
        string detail = body.Value<string>("error") ?? "Request failed";
        if (status == 409) detail += " Re-select the target if the model changed; fetch state again before retrying.";
        if (status == 429) detail += " Use one sequential ARIA action queue with cooldowns.";
        if (status == 503) detail += " Wait for the avatar to finish loading, then test again.";
        if (status == 404) detail += " The ticket may have expired or ARIA restarted.";
        return "HTTP " + status + ": " + detail;
    }
    private static JObject Send(int port, string key, string method, string path, string body, out int status)
    {
        var request = (HttpWebRequest)WebRequest.Create("http://127.0.0.1:" + port + path);
        request.Method = method;
        request.ContentType = "application/json";
        request.Headers["Authorization"] = "Bearer " + key;
        request.Timeout = 3000;
        request.ReadWriteTimeout = 3000;
        request.Proxy = null;
        request.AllowAutoRedirect = false;
        request.KeepAlive = false;
        request.ServicePoint.Expect100Continue = false;
        if (body != null)
        {
            byte[] bytes = Encoding.UTF8.GetBytes(body);
            request.ContentLength = bytes.Length;
            using (var stream = request.GetRequestStream()) stream.Write(bytes, 0, bytes.Length);
        }
        HttpWebResponse response;
        try { response = (HttpWebResponse)request.GetResponse(); }
        catch (WebException ex)
        {
            response = ex.Response as HttpWebResponse;
            if (response == null) throw;
        }
        using (response)
        using (var reader = new StreamReader(response.GetResponseStream(), Encoding.UTF8))
        {
            status = (int)response.StatusCode;
            var text = new StringBuilder();
            char[] buffer = new char[4096];
            int count;
            while ((count = reader.Read(buffer, 0, buffer.Length)) != 0)
            {
                Require(text.Length + count <= 8 * 1024 * 1024, "ARIA response exceeded the connector limit.");
                text.Append(buffer, 0, count);
            }
            return JObject.Parse(text.ToString());
        }
    }
}
