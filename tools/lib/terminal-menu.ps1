function Select-TerminalMenu {
    param(
        [Parameter(Mandatory)][string]$Title,
        [Parameter(Mandatory)][array]$Options,
        [string[]]$Details = @()
    )

    if ([Console]::IsInputRedirected -or [Console]::IsOutputRedirected) {
        throw "The interactive menu requires a terminal. Use command parameters for non-interactive operation."
    }
    if ($Options.Count -eq 0) {
        throw "The interactive menu requires at least one option."
    }

    Clear-Host
    $selected = 0
    $menuTop = [Console]::CursorTop
    $originalCursorVisibility = [Console]::CursorVisible
    [Console]::CursorVisible = $false

    try {
        while ($true) {
            [Console]::SetCursorPosition(0, $menuTop)
            Write-TerminalMenuLine $Title -Color Cyan
            foreach ($detail in $Details) {
                $detailWidth = [Math]::Max(1, [Console]::WindowWidth - 1)
                foreach ($detailLine in (Split-TerminalMenuText -Text $detail -Width $detailWidth)) {
                    Write-TerminalMenuLine $detailLine -Color Red
                }
            }
            Write-TerminalMenuLine "Use Up/Down arrows and Enter. Press Escape to exit." -Color DarkGray
            Write-TerminalMenuLine "" -Color DarkGray

            for ($index = 0; $index -lt $Options.Count; $index++) {
                $prefix = if ($index -eq $selected) { ">" } else { " " }
                $line = "{0} {1,-24} {2}" -f $prefix, $Options[$index].Label, $Options[$index].Description
                $color = if ($index -eq $selected) { "Yellow" } else { "Gray" }
                Write-TerminalMenuLine $line -Color $color
            }

            switch ([Console]::ReadKey($true).Key) {
                "UpArrow" { $selected = ($selected - 1 + $Options.Count) % $Options.Count }
                "DownArrow" { $selected = ($selected + 1) % $Options.Count }
                "Enter" {
                    Clear-Host
                    return $Options[$selected].Value
                }
                "Escape" {
                    Clear-Host
                    return $null
                }
            }
        }
    } finally {
        [Console]::CursorVisible = $originalCursorVisibility
    }
}

function Split-TerminalMenuText {
    param(
        [Parameter(Mandatory)][AllowEmptyString()][string]$Text,
        [Parameter(Mandatory)][ValidateRange(1, 1000)][int]$Width
    )

    $result = [Collections.Generic.List[string]]::new()
    foreach ($logicalLine in ($Text -split "\r?\n")) {
        $remaining = $logicalLine
        while ($remaining.Length -gt $Width) {
            $breakAt = $remaining.LastIndexOf(" ", $Width)
            if ($breakAt -lt [Math]::Min(12, [Math]::Floor($Width / 3))) {
                $breakAt = $Width
            }
            $result.Add($remaining.Substring(0, $breakAt).TrimEnd())
            $remaining = $remaining.Substring($breakAt).TrimStart()
        }
        $result.Add($remaining)
    }
    return $result.ToArray()
}

function Write-TerminalMenuLine {
    param(
        [Parameter(Mandatory)][AllowEmptyString()][string]$Text,
        [Parameter(Mandatory)][ConsoleColor]$Color
    )

    $availableWidth = [Math]::Max(1, [Console]::WindowWidth - 1)
    $visibleText = if ($Text.Length -gt $availableWidth) {
        $Text.Substring(0, [Math]::Max(0, $availableWidth - 3)) + "..."
    } else {
        $Text
    }
    Write-Host $visibleText.PadRight($availableWidth) -ForegroundColor $Color
}
