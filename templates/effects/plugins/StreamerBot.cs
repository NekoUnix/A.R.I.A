// Streamer.bot: Execute C# Code sub-action. Add ariaKey, ariaPort and ariaEffectId
// arguments using Set Argument sub-actions before this code. Never put keys in a
// public action export. No command shell or external package is used.
using System;
using System.IO;
using System.Net;
using System.Text;

public class CPHInline
{
    public bool Execute()
    {
        string key;
        int port;
        ulong id;
        if (!CPH.TryGetArg<string>("ariaKey", out key) || String.IsNullOrWhiteSpace(key))
        { CPH.LogError("ARIA: set the ariaKey argument from Copy API key."); return false; }
        if (!CPH.TryGetArg<int>("ariaPort", out port)) port = 39421;
        if (!CPH.TryGetArg<ulong>("ariaEffectId", out id) || id == 0 || port < 1024 || port > 65535)
        { CPH.LogError("ARIA: set a valid ariaEffectId and port."); return false; }
        try
        {
            var request = (HttpWebRequest)WebRequest.Create("http://127.0.0.1:" + port + "/v1/effects/trigger");
            request.Method = "POST";
            request.ContentType = "application/json";
            request.Headers["Authorization"] = "Bearer " + key.Trim();
            request.Timeout = 3000;
            request.ReadWriteTimeout = 3000;
            request.Proxy = null;
            request.ServicePoint.Expect100Continue = false;
            var data = Encoding.UTF8.GetBytes("{\"version\":1,\"id\":" + id + "}");
            request.ContentLength = data.Length;
            using (var stream = request.GetRequestStream()) stream.Write(data, 0, data.Length);
            using (var response = (HttpWebResponse)request.GetResponse())
                return response.StatusCode == HttpStatusCode.Accepted;
        }
        catch (WebException e)
        { CPH.LogError("ARIA effect failed: " + e.Status + ". Check ARIA's local API and effect status."); return false; }
    }
}
