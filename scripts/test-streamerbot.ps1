param([Parameter(Mandatory=$true)][int]$Port, [Parameter(Mandatory=$true)][string]$Results)
$ErrorActionPreference='Stop'
$ariaCode=[IO.File]::ReadAllText((Join-Path $PSScriptRoot '../templates/streamerbot/AriaConnector.cs')).Replace("`r`n","`n")
$ariaCode=$ariaCode.Replace("public class CPHInline`n{", "public class CPHInline`n{`n    public TestCPH CPH = new TestCPH();")
if (-not $ariaCode.Contains('public TestCPH CPH')) { throw 'Could not inject test CPH adapter' }
$ariaCode+=@'
public class TestCPH {
    public System.Collections.Generic.Dictionary<string,object> Args = new System.Collections.Generic.Dictionary<string,object>();
    public System.Collections.Generic.Dictionary<string,object> Globals = new System.Collections.Generic.Dictionary<string,object>();
    public System.Collections.Generic.List<string> Logs = new System.Collections.Generic.List<string>();
    public T GetGlobalVar<T>(string name, bool persisted) { object value; return Globals.TryGetValue(name,out value) ? (T)value : default(T); }
    public bool TryGetArg<T>(string name, out T value) { object found; if(Args.TryGetValue(name,out found)) { value=(T)found;return true; } value=default(T);return false; }
    public void SetArgument(string name,object value) {Args[name]=value;}
    public void LogError(string value) {Logs.Add(value);}
    public void LogInfo(string value) {Logs.Add(value);}
}
'@
$ariaRefs=@(Get-ChildItem (Join-Path $PSHOME 'ref') -Filter '*.dll' | ForEach-Object FullName)
$ariaRefs+=[Newtonsoft.Json.Linq.JObject].Assembly.Location
Add-Type -TypeDefinition ("#pragma warning disable SYSLIB0014`n"+$ariaCode) -ReferencedAssemblies $ariaRefs -CompilerOptions "/nowarn:1701"
$ariaResults=@()
foreach($ariaCase in @('connected','applied','rejected','stale','pending','malformed','missing_key')) {
    $null=Invoke-RestMethod -Uri "http://127.0.0.1:$Port/_case/$ariaCase"
    $ariaClient=[CPHInline]::new()
    $ariaClient.CPH.Globals['ariaPort']=$Port
    if($ariaCase -ne 'missing_key') {$ariaClient.CPH.Globals['ariaApiKey']='a'*48}
    if($ariaCase -ne 'connected') {$ariaClient.CPH.Args['ariaAction']='{"type":"workspace_action","target":{"Avatar":{"profile":42,"command":{"Item":3}}},"mode":"Off"}'}
    if($ariaCase -eq 'malformed') {$ariaClient.CPH.Args['ariaAction']='{broken'}
    $ariaClient.CPH.Args['ariaWaitSeconds']=1
    $ariaSuccess=$ariaClient.Execute()
    $ariaServer=Invoke-RestMethod -Uri "http://127.0.0.1:$Port/_counts"
    if(($ariaClient.CPH.Logs -join ' ').Contains('a'*48)) {throw 'Connector logged its key'}
    $ariaResults+=@{scenario=$ariaCase;success=$ariaSuccess;status=$ariaClient.CPH.Args['ariaStatus'];ticket=$ariaClient.CPH.Args['ariaTicket'];posts=$ariaServer.posts;error=$ariaClient.CPH.Args['ariaError']}
}
$ariaResults | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Results -Encoding utf8
