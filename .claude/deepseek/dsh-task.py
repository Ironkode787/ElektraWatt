#!/usr/bin/env python3
"""Run the DeepSeek harness (dsh) as the coder for a Claude Code orchestrator.

Each task gets its own git worktree (.claude/worktrees/ds-<name> on branch ds/<name>) and a run directory
(.claude/deepseek/runs/<name>/) holding the task texts, dsh's NDJSON event logs and state.json. A round runs
`dsh --profile headless --json` in the worktree, then prints a compact summary for the orchestrator: the outcome,
token use, the files changed against the base commit and the coder's final report. `resume` continues the same dsh
session with revision instructions. The coder never commits; the orchestrator reviews, commits and merges.

  python .claude/deepseek/dsh-task.py start <name> --task-file brief.md [--base REF] [--model pro|flash|ID]
                                      [--effort high|max] [--sandbox workspace|full] [--timeout-min N]
  python .claude/deepseek/dsh-task.py resume <name> --task-file fixes.md [same options]
  python .claude/deepseek/dsh-task.py show <name> [--round N]
  python .claude/deepseek/dsh-task.py list
  python .claude/deepseek/dsh-task.py remove <name> [--keep-branch]

A task file of `-` reads the task from stdin. The DeepSeek API key comes from DEEPSEEK_API_KEY in the environment
or in $DSH_HOME/.env (default ~/.dsh/.env).
"""
from __future__ import annotations

import argparse
import collections
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROVIDER = 'deepseek-official'
MODELS = {'pro': 'deepseek-v4-pro', 'flash': 'deepseek-flash'}
SANDBOXES = {'workspace': 'workspace-write', 'full': 'danger-full-access'}
# The project's build settings (CLAUDE.md "Disk and build concurrency"); set when the caller has not.
CARGO_ENV = {'CARGO_PROFILE_DEV_DEBUG': 'line-tables-only', 'CARGO_INCREMENTAL': '0', 'CARGO_BUILD_JOBS': '4'}
NAME_RE = re.compile(r'^[a-z0-9][a-z0-9-]{0,47}$')
REPORT_LINES = 150
FOOTER = ('\n\n---\nYou are running in a git worktree prepared for this task. Follow .claude/deepseek/CODER.md '
          '(the instructions loaded for this project) and end with the `## Report` it describes.\n')


def die(msg: str, code: int = 2) -> None:
    print(f'dsh-task: {msg}', file=sys.stderr)
    raise SystemExit(code)


def git(*args: str, cwd: Path, check: bool = True) -> str:
    r = subprocess.run(['git', *args], cwd=cwd, capture_output=True, text=True, encoding='utf-8', errors='replace')
    if check and r.returncode:
        die(f"git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout


def main_checkout() -> Path:
    return Path(git('rev-parse', '--path-format=absolute', '--git-common-dir', cwd=HERE).strip()).parent


def run_dir(name: str) -> Path:
    return main_checkout() / '.claude' / 'deepseek' / 'runs' / name


def load_state(name: str) -> dict:
    path = run_dir(name) / 'state.json'
    if not path.exists():
        die(f'no run named {name!r} (see `list`)')
    return json.loads(path.read_text(encoding='utf-8'))


def save_state(state: dict) -> None:
    path = run_dir(state['name']) / 'state.json'
    path.write_text(json.dumps(state, indent=2) + '\n', encoding='utf-8')


def dsh_command() -> list[str]:
    exe = shutil.which('dsh')
    if not exe:
        die('dsh not found; install it with `npm install -g @deepseek-ai/dsh`')
    if exe.lower().endswith(('.cmd', '.bat')):
        # npm's Windows shim runs through cmd.exe; call node on the package entry instead to keep argv intact.
        entry = Path(exe).parent / 'node_modules' / '@deepseek-ai' / 'dsh' / 'lib' / 'bin.js'
        node = Path(exe).parent / 'node.exe'
        return [str(node) if node.exists() else 'node', str(entry)]
    return [exe]


def check_api_key() -> None:
    if os.environ.get('DEEPSEEK_API_KEY'):
        return
    env_file = Path(os.environ.get('DSH_HOME') or Path.home() / '.dsh') / '.env'
    if env_file.exists() and re.search(r'^\s*DEEPSEEK_API_KEY\s*=\s*\S', env_file.read_text(encoding='utf-8'),
                                       re.M):
        return
    die(f'no DeepSeek API key: set DEEPSEEK_API_KEY or add `DEEPSEEK_API_KEY=...` to {env_file}')


def read_task(path: str) -> str:
    text = sys.stdin.read() if path == '-' else Path(path).read_text(encoding='utf-8')
    if not text.strip():
        die('the task is empty')
    return text


def stop_tree(proc: subprocess.Popen) -> None:
    if os.name == 'nt':
        subprocess.run(['taskkill', '/PID', str(proc.pid), '/T', '/F'], capture_output=True)
    else:
        proc.kill()


def run_round(state: dict, task: str, args: argparse.Namespace) -> int:
    rdir = run_dir(state['name'])
    wt = Path(state['worktree'])
    n = len(state['rounds']) + 1
    model = MODELS.get(args.model, args.model)
    if not (wt / '.claude' / 'deepseek' / 'CODER.md').exists():
        # The base predates the coder brief, so dsh cannot load it from the worktree: send it with the task.
        task = (HERE / 'CODER.md').read_text(encoding='utf-8') + '\n\n# Task\n\n' + task
    task += FOOTER
    (rdir / f'task-{n}.md').write_text(task, encoding='utf-8')
    overlay = rdir / f'model-{n}.patch.yml'
    overlay.write_text(f'- id: agent-default-model\n  config:\n    provider: {PROVIDER}\n    model: {model}\n'
                       + (f'    reasoningEffort: {args.effort}\n' if args.effort else ''), encoding='utf-8')
    cmd = [*dsh_command(), '--profile', 'headless', '--patch', str(HERE / 'headless.patch.yml'),
           '--patch', str(overlay), '--json', '--permission-mode', SANDBOXES[args.sandbox],
           '--approval-policy', 'never']
    if state.get('session_id'):
        cmd += ['--session-id', state['session_id']]
    cmd.append('-')
    env = dict(os.environ)
    for key, value in CARGO_ENV.items():
        env.setdefault(key, value)

    events, stderr = rdir / f'events-{n}.jsonl', rdir / f'stderr-{n}.log'
    rnd = {'round': n, 'model': model, 'effort': args.effort, 'sandbox': SANDBOXES[args.sandbox],
           'started': datetime.datetime.now().isoformat(timespec='seconds'), 'status': 'running'}
    state['rounds'].append(rnd)
    t0 = time.monotonic()
    with open(events, 'wb') as out, open(stderr, 'wb') as err:
        proc = subprocess.Popen(cmd, cwd=wt, stdin=subprocess.PIPE, stdout=out, stderr=err, env=env)
        rnd['pid'] = proc.pid
        save_state(state)
        proc.stdin.write(task.encode('utf-8'))
        proc.stdin.close()
        try:
            code = proc.wait(timeout=args.timeout_min * 60)
            timed_out = False
        except subprocess.TimeoutExpired:
            stop_tree(proc)
            code, timed_out = proc.wait(), True
    info = parse_events(events)
    if info['session'] and not state.get('session_id'):
        state['session_id'] = info['session']
    ok = code == 0 and (info['reason'] or {}).get('kind') == 'completed'
    rnd.update(exit=code, timed_out=timed_out, seconds=round(time.monotonic() - t0),
               status='completed' if ok else 'timed-out' if timed_out else 'failed')
    rnd.pop('pid', None)
    save_state(state)
    print_summary(state, n)
    return 0 if ok else 1


def parse_events(path: Path) -> dict:
    info = {'session': None, 'reason': None, 'final': None, 'error': None, 'tools': collections.Counter(),
            'tool_errors': 0, 'tokens': collections.Counter(), 'steps': 0, 'last_text': None}
    if not path.exists():
        return info
    for line in path.read_text(encoding='utf-8', errors='replace').splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        kind = ev.get('type')
        if kind == 'session':
            info['session'] = ev.get('sessionId')
        elif kind == 'tool_call':
            info['tools'][ev.get('tool') or '?'] += 1
        elif kind == 'tool_result' and ev.get('status') == 'error':
            info['tool_errors'] += 1
        elif kind == 'status' and ev.get('phase') == 'step_end':
            info['steps'] += 1
            for key, value in (ev.get('usage') or {}).items():
                if isinstance(value, int):
                    info['tokens'][key] += value
        elif kind == 'text':
            info['last_text'] = ev.get('text')
        elif kind == 'final':
            info['session'] = ev.get('sessionId') or info['session']
            info['reason'], info['final'] = ev.get('reason'), ev.get('text')
        elif kind == 'error':
            info['error'] = ev.get('message') or json.dumps(ev)[:500]
    return info


def changes(state: dict) -> str:
    wt = Path(state['worktree'])
    if not wt.exists():
        return '(worktree removed)'
    head = git('rev-parse', 'HEAD', cwd=wt).strip()
    lines = []
    if head != state['base']:
        lines.append(f'!! HEAD moved from {state["base"][:10]} to {head[:10]} (the coder committed)')
    stat = git('diff', '--stat=120', state['base'], cwd=wt).rstrip()
    untracked = git('ls-files', '--others', '--exclude-standard', cwd=wt).split()
    lines += [stat] if stat else []
    lines += [f' ?? {path}' for path in untracked]
    return '\n'.join(lines) or '(no changes)'


def print_summary(state: dict, n: int) -> None:
    rdir = run_dir(state['name'])
    rnd = state['rounds'][n - 1]
    info = parse_events(rdir / f'events-{n}.jsonl')
    reason = info['reason'] or {}
    tools = ', '.join(f'{k} {v}' for k, v in info['tools'].most_common()) or 'none'
    tok = info['tokens']
    print(f"== dsh run {state['name']} round {n}: {rnd['status']} (exit {rnd.get('exit', '-')}, "
          f"reason {reason.get('kind', 'none')}), session {state.get('session_id') or info['session']}")
    print(f"model {rnd['model']}{' / ' + rnd['effort'] if rnd.get('effort') else ''}, sandbox {rnd['sandbox']}, "
          f"{rnd.get('seconds', '?')} s, {info['steps']} steps, tokens in {tok['inputTokens']} "
          f"(cache read {tok['cacheReadTokens']}) out {tok['outputTokens']}")
    print(f"tools: {tools}; {info['tool_errors']} tool errors")
    if info['error'] or reason.get('kind') == 'error':
        print(f"error: {info['error'] or json.dumps(reason)[:500]}")
    print(f"worktree {state['worktree']} (branch {state['branch']}, base {state['base'][:10]})")
    print('-- changes against base --')
    print(changes(state))
    print('-- coder report --')
    report = (info['final'] or info['last_text'] or '(no final message)').strip().splitlines()
    print('\n'.join(report[-REPORT_LINES:]))
    if len(report) > REPORT_LINES:
        print(f'(report cut to its last {REPORT_LINES} of {len(report)} lines)')
    print(f'-- logs: {rdir / f"events-{n}.jsonl"} , {rdir / f"stderr-{n}.log"}')


def cmd_start(args: argparse.Namespace) -> int:
    if not NAME_RE.match(args.name):
        die('a run name is lower-case letters, digits and dashes, up to 48 characters')
    check_api_key()
    task = read_task(args.task_file)
    root = main_checkout()
    rdir = run_dir(args.name)
    if rdir.exists():
        die(f'run {args.name!r} exists; use `resume` or `remove` it first')
    if args.worktree:
        wt = Path(args.worktree).resolve()
        branch = git('rev-parse', '--abbrev-ref', 'HEAD', cwd=wt).strip()
        base = git('rev-parse', 'HEAD', cwd=wt).strip()
    else:
        wt = root / '.claude' / 'worktrees' / f'ds-{args.name}'
        branch = f'ds/{args.name}'
        base = git('rev-parse', '--verify', f'{args.base}^{{commit}}', cwd=root).strip()
        git('worktree', 'add', '-b', branch, str(wt), base, cwd=root)
    rdir.mkdir(parents=True)
    state = {'name': args.name, 'worktree': str(wt), 'branch': branch, 'base': base, 'session_id': None,
             'created': datetime.datetime.now().isoformat(timespec='seconds'), 'rounds': []}
    save_state(state)
    return run_round(state, task, args)


def cmd_resume(args: argparse.Namespace) -> int:
    check_api_key()
    state = load_state(args.name)
    if not state.get('session_id'):
        die('the first round recorded no dsh session; start a new run instead')
    if any(r['status'] == 'running' for r in state['rounds']):
        die('a round is still marked running; wait for it (or clear it with `show` once its process has exited)')
    return run_round(state, read_task(args.task_file), args)


def cmd_show(args: argparse.Namespace) -> int:
    state = load_state(args.name)
    if not state['rounds']:
        die('the run has no rounds')
    n = args.round or len(state['rounds'])
    rnd = state['rounds'][n - 1]
    if rnd['status'] == 'running' and not pid_alive(rnd.get('pid')):
        rnd['status'] = 'interrupted'
        rnd.pop('pid', None)
        save_state(state)
    print_summary(state, n)
    return 0


def pid_alive(pid: int | None) -> bool:
    if not pid:
        return False
    if os.name == 'nt':
        out = subprocess.run(['tasklist', '/FI', f'PID eq {pid}', '/NH'], capture_output=True, text=True).stdout
        return str(pid) in out
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def cmd_list(_args: argparse.Namespace) -> int:
    base = main_checkout() / '.claude' / 'deepseek' / 'runs'
    for path in sorted(base.glob('*/state.json')) if base.exists() else []:
        state = json.loads(path.read_text(encoding='utf-8'))
        last = state['rounds'][-1] if state['rounds'] else {}
        print(f"{state['name']:<30} {len(state['rounds'])} round(s), last {last.get('status', '-'):<11} "
              f"{state['branch']}  {state['worktree']}")
    return 0


def cmd_remove(args: argparse.Namespace) -> int:
    state = load_state(args.name)
    root = main_checkout()
    wt = Path(state['worktree'])
    if wt.exists() and wt.parent == root / '.claude' / 'worktrees':
        git('worktree', 'remove', '--force', str(wt), cwd=root)
        if not args.keep_branch and state['branch'].startswith('ds/'):
            git('branch', '-D', state['branch'], cwd=root, check=False)
    shutil.rmtree(run_dir(args.name))
    print(f"removed run {args.name}" + ('' if wt.parent == root / '.claude' / 'worktrees'
                                       else f' (left the caller-owned worktree {wt} in place)'))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    sub = parser.add_subparsers(dest='cmd', required=True)

    def round_options(p: argparse.ArgumentParser) -> None:
        p.add_argument('name')
        p.add_argument('--task-file', required=True, help='the task text, or - for stdin')
        p.add_argument('--model', default='pro', help='pro (deepseek-v4-pro), flash (deepseek-flash) or a model id')
        p.add_argument('--effort', default='high', help='reasoning effort passed to the DeepSeek adapter')
        p.add_argument('--sandbox', choices=SANDBOXES, default='workspace',
                       help='workspace: writes confined to the worktree; full: no sandbox')
        p.add_argument('--timeout-min', type=float, default=90, help='stop the round after this many minutes')

    p = sub.add_parser('start', help='create the worktree and run the first round')
    round_options(p)
    p.add_argument('--base', default='HEAD', help='commit the new worktree starts from')
    p.add_argument('--worktree', help='use this existing worktree instead of creating one')
    p.set_defaults(func=cmd_start)
    p = sub.add_parser('resume', help='send revision instructions to the same dsh session')
    round_options(p)
    p.set_defaults(func=cmd_resume)
    p = sub.add_parser('show', help='print the summary of a round (the last by default)')
    p.add_argument('name')
    p.add_argument('--round', type=int)
    p.set_defaults(func=cmd_show)
    p = sub.add_parser('list', help='list the runs')
    p.set_defaults(func=cmd_list)
    p = sub.add_parser('remove', help='remove the run, its worktree and its ds/ branch')
    p.add_argument('name')
    p.add_argument('--keep-branch', action='store_true')
    p.set_defaults(func=cmd_remove)
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')  # the coder's report is not limited to the console code page
    return args.func(args)


if __name__ == '__main__':
    sys.exit(main())
