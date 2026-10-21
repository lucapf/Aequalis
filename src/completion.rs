//! Bash and zsh completion scripts, generated from the unit tables in `Config`.
//!
//! The scripts also install an Enter hook: a typed `aequalis (10+12-3)/4Km Mi` would be rejected
//! by the shell (`(` is shell syntax, `*` a glob), so before running it the hook asks
//! `aequalis --quote-line` to rewrite it as `aequalis '(10+12-3)/4Km Mi'`.

use crate::calc;
use crate::config::Config;

pub const SHELLS: &[&str] = &["bash", "zsh"];
const FLAGS: &str = "--precision --silent --json --list --autocompletion --help --version";

const BASH: &str = r#"# bash completion for aequalis
# Enable with: eval "$(aequalis --autocompletion bash)"
_aequalis() {
    local cur prev w i pos=0 expr=0 units=0
    # Words of an arithmetic expression (`100 + 20`, `[1+2]/3`) fill a single value slot.
    local num='^[-+]?[0-9.([{]' op='^([-+*/^x]|\*\*)$'
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    COMPREPLY=()
    case "$prev" in
        --autocompletion) COMPREPLY=( $(compgen -W "@SHELLS@" -- "$cur") ); return 0 ;;
        --precision) return 0 ;;
    esac
    if [[ "$cur" == -* && ! "$cur" =~ $num ]]; then
        COMPREPLY=( $(compgen -W "@FLAGS@" -- "$cur") )
        return 0
    fi
    for (( i=1; i<COMP_CWORD; i++ )); do
        w="${COMP_WORDS[i]}"
        if [[ "$w" == --precision ]]; then (( i++ )); continue; fi
        if [[ "$w" == -* && ! "$w" =~ $num ]]; then continue; fi
        if (( units == 0 )) && [[ "$w" =~ $num || "$w" =~ $op ]]; then expr=1; else (( units++ )); fi
    done
    pos=$(( expr + units ))
    if (( pos == 1 || pos == 2 )); then
        COMPREPLY=( $(compgen -W "@UNITS@" -- "$cur") )
    fi
}
complete -F _aequalis aequalis

# Enter hook: quote aequalis expressions, so `aequalis (1+2)*3 km mi` reaches the program intact.
_aequalis_quote_line() {
    [[ $READLINE_LINE == *aequalis*[\(\)\*\[\]^]* ]] || return 0
    local quoted
    quoted=$(@BIN@ --quote-line "$READLINE_LINE" 2>/dev/null) || return 0
    READLINE_LINE=$quoted
    READLINE_POINT=${#READLINE_LINE}
}
# READLINE_LINE needs bash 4+ (macOS ships bash 3.2: completion works, the hook is skipped).
if [[ $- == *i* ]] && (( BASH_VERSINFO[0] >= 4 )); then
    for _aequalis_keymap in emacs vi-insert; do
        bind -m "$_aequalis_keymap" -x '"\C-x\C-q\C-q": _aequalis_quote_line'
        bind -m "$_aequalis_keymap" '"\C-x\C-q\C-a": accept-line'
        bind -m "$_aequalis_keymap" '"\C-m": "\C-x\C-q\C-q\C-x\C-q\C-a"'
        bind -m "$_aequalis_keymap" '"\C-j": "\C-x\C-q\C-q\C-x\C-q\C-a"'
    done
    unset _aequalis_keymap
fi
"#;

const ZSH: &str = r#"#compdef aequalis
# zsh completion for aequalis
# Enable with: eval "$(aequalis --autocompletion zsh)"  (after compinit)
# or save as _aequalis in a directory on $fpath.
_aequalis() {
    local -a units flags
    local w i pos=0 expr=0 nunits=0
    # Words of an arithmetic expression (`100 + 20`, `[1+2]/3`) fill a single value slot.
    local num='^[-+]?[0-9.([{]' op='^([-+*/^x]|\*\*)$'
    units=(@UNITS@)
    flags=(@FLAGS@)
    case ${words[CURRENT-1]} in
        --autocompletion) compadd -- @SHELLS@; return ;;
        --precision) return ;;
    esac
    if [[ $PREFIX == -* && ! $PREFIX =~ $num ]]; then
        compadd -- $flags
        return
    fi
    for (( i=2; i<CURRENT; i++ )); do
        w=${words[i]}
        if [[ $w == --precision ]]; then (( i++ )); continue; fi
        if [[ $w == -* && ! $w =~ $num ]]; then continue; fi
        if (( nunits == 0 )) && [[ $w =~ $num || $w =~ $op ]]; then expr=1; else (( nunits++ )); fi
    done
    pos=$(( expr + nunits ))
    if (( pos == 1 || pos == 2 )); then
        compadd -- $units
    fi
}
# Enter hook: quote aequalis expressions, so `aequalis (1+2)*3 km mi` reaches the program intact.
_aequalis_accept_line() {
    if [[ $BUFFER == *aequalis*[\(\)\*\[\]^]* ]]; then
        local quoted
        quoted=$(@BIN@ --quote-line "$BUFFER" 2>/dev/null) && BUFFER=$quoted
    fi
    zle .accept-line
}
if [[ ${zsh_eval_context[-1]} == loadautofunc ]]; then
    _aequalis "$@"
else
    compdef _aequalis aequalis
    [[ -o interactive ]] && zle -N accept-line _aequalis_accept_line
fi
"#;

/// Characters the shell would interpret inside an expression: `(` `)` are syntax, `*` `[` `]` `^` globs.
const SHELL_SPECIAL: &[char] = &['(', ')', '*', '[', ']', '^'];
/// A line with any of these is left alone: it is already quoted, or uses shell features
/// (variables, pipes, redirections, several commands, comments) that quoting could break.
const UNTOUCHABLE: &[char] = &['\'', '"', '\\', '$', '`', ';', '&', '|', '<', '>', '#', '\n'];

/// Rewrites a typed `aequalis ...` line so the shell passes the expression through unchanged:
/// `aequalis (10+12-3)/4Km Mi` becomes `aequalis '(10+12-3)/4Km Mi'`. The expression and its units
/// become one argument (aequalis joins its words with spaces anyway); flags stay outside the
/// quotes. Returns `None` when there is nothing to quote or the line must not be touched.
pub fn quote_line(line: &str) -> Option<String> {
    if line.contains(UNTOUCHABLE) {
        return None;
    }
    let body = line.trim_start();
    let indent = &line[..line.len() - body.len()];
    let words: Vec<&str> = body.split_whitespace().collect();
    let (cmd, args) = words.split_first()?;
    if *cmd != "aequalis" && !cmd.ends_with("/aequalis") {
        return None;
    }
    let is_flag = |w: &str| w.len() > 1 && w.starts_with('-') && w != "->" && !calc::starts_expression(w);
    let takes_value = |w: &str| matches!(w, "--precision" | "--autocompletion" | "--quote-line");
    // Flags before the expression (with their values), then the expression and units, then the rest.
    let mut start = 0;
    while start < args.len() && is_flag(args[start]) {
        start += 1 + usize::from(takes_value(args[start]));
    }
    let start = start.min(args.len());
    let end = args[start..].iter().position(|w| is_flag(w)).map_or(args.len(), |p| start + p);
    let expr = args[start..end].join(" ");
    if !expr.contains(SHELL_SPECIAL) {
        return None;
    }
    let quoted = format!("'{expr}'");
    let all: Vec<&str> = [*cmd].into_iter().chain(args[..start].iter().copied()).chain([quoted.as_str()]).chain(args[end..].iter().copied()).collect();
    Some(format!("{indent}{}", all.join(" ")))
}

/// The running binary as a single-quoted shell word, so the hooks call this exact aequalis.
fn self_path() -> String {
    let path = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "aequalis".into());
    format!("'{}'", path.replace('\'', r"'\''"))
}

/// Returns the completion script for `shell`, or `None` if the shell is unsupported.
pub fn script(config: &Config, shell: &str) -> Option<String> {
    let template = match shell {
        "bash" => BASH,
        "zsh" => ZSH,
        _ => return None,
    };
    // Unit symbols, then the date words (`now`, `today`, the formats, the date patterns).
    let patterns = crate::time::pattern_words();
    let units: Vec<&str> = config
        .categories
        .iter()
        .flat_map(|c| c.units.iter().map(|u| u.symbol))
        .chain(crate::config::DATE_WORDS.iter().copied())
        .chain(patterns.iter().map(String::as_str))
        .chain(["num"])
        .collect();
    Some(template.replace("@UNITS@", &units.join(" ")).replace("@FLAGS@", FLAGS).replace("@SHELLS@", &SHELLS.join(" ")).replace("@BIN@", &self_path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_contain_units_and_flags() {
        let config = Config::default();
        for shell in SHELLS {
            let s = script(&config, shell).unwrap();
            for placeholder in ["@UNITS@", "@FLAGS@", "@SHELLS@", "@BIN@"] {
                assert!(!s.contains(placeholder), "unreplaced {placeholder} in {shell} script");
            }
            for cat in config.categories {
                for u in cat.units {
                    let mut words = s.split(|c: char| !c.is_ascii_alphanumeric() && !"-/".contains(c));
                    assert!(words.any(|w| w == u.symbol), "{shell}: missing {}", u.symbol);
                }
            }
            for flag in FLAGS.split(' ') {
                assert!(s.contains(flag), "{shell}: missing {flag}");
            }
        }
        assert!(script(&config, "fish").is_none());
        for shell in SHELLS {
            let s = script(&config, shell).unwrap();
            for word in crate::config::DATE_WORDS {
                assert!(s.contains(&format!(" {word} ")) || s.contains(&format!(" {word}\"")) || s.contains(&format!("({word} ")), "{shell}: {word}");
            }
        }
    }

    fn quote(line: &str) -> Option<String> {
        quote_line(line)
    }

    #[test]
    fn quotes_expressions() {
        assert_eq!(quote("aequalis (10+12-3)/4Km Mi").unwrap(), "aequalis '(10+12-3)/4Km Mi'");
        assert_eq!(quote("./aequalis (10+12-3)/4Km Mi").unwrap(), "./aequalis '(10+12-3)/4Km Mi'");
        assert_eq!(quote("target/release/aequalis 2 * 3").unwrap(), "target/release/aequalis '2 * 3'");
        assert_eq!(quote("aequalis 10**2").unwrap(), "aequalis '10**2'");
        assert_eq!(quote("aequalis [1+2]/3 km mi").unwrap(), "aequalis '[1+2]/3 km mi'");
        assert_eq!(quote("  aequalis (1+2) km m").unwrap(), "  aequalis '(1+2) km m'");
        assert_eq!(quote("aequalis -(2+3)").unwrap(), "aequalis '-(2+3)'");
    }

    #[test]
    fn keeps_flags_outside_the_quotes() {
        assert_eq!(quote("aequalis (1+2) km m --precision 2").unwrap(), "aequalis '(1+2) km m' --precision 2");
        assert_eq!(quote("aequalis -s --precision 2 (1+2)*3").unwrap(), "aequalis -s --precision 2 '(1+2)*3'");
        assert_eq!(quote("aequalis --precision=1 1/(3) -s").unwrap(), "aequalis --precision=1 '1/(3)' -s");
    }

    #[test]
    fn leaves_other_lines_alone() {
        for line in [
            "aequalis 10 km mi",         // nothing to quote
            "aequalis {1+2}/3 km mi",    // braces are safe unquoted
            "aequalis '(1+2)' km m",     // already quoted
            "aequalis \\(1+2\\) km m",   // already escaped
            "aequalis (1+2) km m | cat", // pipeline
            "aequalis $(echo 1) km m",   // command substitution
            "aequalis (1) km m; ls *",   // several commands
            "echo (1+2)",                // another command
            "aequalisx (1+2)",           // another command
            "ls aequalis*",              // `aequalis` is not the command
            "aequalis --precision 2",    // no expression
            "",
        ] {
            assert_eq!(quote(line), None, "{line}");
        }
    }

    #[test]
    fn scripts_install_the_enter_hook() {
        let config = Config::default();
        let bash = script(&config, "bash").unwrap();
        assert!(bash.contains("--quote-line") && bash.contains(r#"bind -m "$_aequalis_keymap" '"\C-m""#));
        let zsh = script(&config, "zsh").unwrap();
        assert!(zsh.contains("--quote-line") && zsh.contains("zle -N accept-line _aequalis_accept_line"));
    }
}
