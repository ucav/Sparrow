# Sparrow PowerShell completion
Register-ArgumentCompleter -Native -CommandName sparrow -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $commands = @('run','chat','swarm','schedule','model','auth','agent','skills','mcp','checkpoint','rewind','replay','gateway','profile','import','config','update','doctor','setup','learn','memory','tui','console')
    $flags = @('--tui','--web','--json','--model','--local','--budget','--autonomy','--sandbox','--profile','--agent','--help','--version')
    $all = $commands + $flags
    $all | Where-Object { $_ -like "$wordToComplete*" }
}
