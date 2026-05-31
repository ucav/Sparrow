# Sparrow bash completion
_sparrow() {
    local cur prev words cword
    _init_completion || return
    local commands="run chat swarm schedule model auth agent skills mcp checkpoint rewind replay gateway profile import config update doctor setup learn memory tui console"
    local flags="--tui --web --json --model --local --budget --autonomy --sandbox --profile --agent --no-checkpoint --help --version"
    if [[ $cword -eq 1 ]]; then
        COMPREPLY=($(compgen -W "$commands $flags" -- "$cur"))
    fi
}
complete -F _sparrow sparrow
