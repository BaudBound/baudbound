function Select-DevelopmentAction {
    $options = @(
        [PSCustomObject]@{ Value = "Desktop"; Label = "Desktop app"; Description = "Launch Tauri, Vite, and the Rust desktop runner." },
        [PSCustomObject]@{ Value = "DesktopUi"; Label = "Desktop UI only"; Description = "Start the Vite frontend at http://127.0.0.1:1420." },
        [PSCustomObject]@{ Value = "Editor"; Label = "Editor"; Description = "Start the Next.js editor at http://localhost:3000." },
        [PSCustomObject]@{ Value = "Service"; Label = "Runner service"; Description = "Run long-lived trigger listeners in the foreground." },
        [PSCustomObject]@{ Value = "Status"; Label = "Runner status"; Description = "Print current runner and background-service health." },
        [PSCustomObject]@{ Value = "Install"; Label = "Install dependencies"; Description = "Install exact locked editor and desktop UI packages." },
        [PSCustomObject]@{ Value = "Checks"; Label = "Lint and typecheck"; Description = "Run Rust, editor, schema, and UI static checks." },
        [PSCustomObject]@{ Value = "Tests"; Label = "Tests"; Description = "Run Rust, editor contract, and desktop UI tests." },
        [PSCustomObject]@{ Value = "EditorE2E"; Label = "Editor browser tests"; Description = "Run the Playwright end-to-end test suite." },
        [PSCustomObject]@{ Value = "Schemas"; Label = "Generate schemas"; Description = "Regenerate and verify public node schemas." },
        [PSCustomObject]@{ Value = "Build"; Label = "Build applications"; Description = "Build the runner, desktop UI, and editor." },
        [PSCustomObject]@{ Value = $null; Label = "Exit"; Description = "Close the development helper." }
    )
    return Select-TerminalMenu -Title "BaudBound development" -Options $options
}

function Wait-ForDevelopmentMenu {
    Write-Host ""
    Write-Host "Press any key to return to the development menu." -ForegroundColor DarkGray
    [Console]::ReadKey($true) | Out-Null
}
