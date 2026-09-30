# tb shell integration. In ~/.bashrc:  source /path/to/treebeard/tb.bash
#
# q in tb leaves this shell in the folder under the cursor (a file: its folder);
# esc / ctrl-c leave it where it was. Inside tb, ! runs a command and s opens a
# shell in that folder; tb resumes where it was when they exit.
tb() {
    local f rc d
    f=$(mktemp) || return
    command tb --cwd-file "$f" "$@"
    rc=$?
    d=$(<"$f")
    rm -f -- "$f"
    [[ -n $d && -d $d && $d != "$PWD" ]] && cd -- "$d"
    return $rc
}
