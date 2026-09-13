using System;
using System.Diagnostics;
using System.IO;
using System.Linq;

internal static class FedoraWinLauncher
{
    private static string Quote(string value)
    {
        if (String.IsNullOrEmpty(value)) return "\"\"";
        return "\"" + value.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";
    }

    [STAThread]
    private static int Main(string[] args)
    {
        string root = AppDomain.CurrentDomain.BaseDirectory;
        string script = Path.Combine(root, "FedoraWin.ps1");
        if (!File.Exists(script)) return 3;
        string forwarded = String.Join(" ", args.Select(Quote));
        var psi = new ProcessStartInfo {
            FileName = "powershell.exe",
            Arguments = "-NoLogo -NoProfile -ExecutionPolicy Bypass -STA -WindowStyle Hidden -File " + Quote(script) + (String.IsNullOrWhiteSpace(forwarded) ? "" : " " + forwarded),
            WorkingDirectory = root,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        using (var process = Process.Start(psi)) {
            if (process == null) return 4;
            process.WaitForExit();
            return process.ExitCode;
        }
    }
}