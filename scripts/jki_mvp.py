# /// script
# dependencies = [
#   "pyotp",
#   "pyperclip",
#   "typer",
#   "rich",
# ]
# ///
import json
import sys
import os
from typing import List, Optional
import pyotp
import pyperclip
import typer
from rich.console import Console

app = typer.Typer(add_completion=False, no_args_is_help=False)
console = Console(stderr=True)

def fuzzy_match(pattern, target):
    pattern = pattern.lower()
    target = target.lower()
    it = iter(target)
    return all(c in it for c in pattern)

def search_accounts(accounts, patterns):
    results = []
    for acc in accounts:
        target_str = f"{acc.get('issuer') or ''} {acc.get('name')}".lower()
        if all(fuzzy_match(p, target_str) for p in patterns):
            results.append(acc)
    return results

@app.command()
def main(
    patterns: List[str] = typer.Argument(None, help="Search patterns"),
    force_list: bool = typer.Option(False, "--list", help="Force listing results"),
):
    # Load data
    vault_path = "data/private/vault.json"
    if not os.path.exists(vault_path):
        console.print(f"[red]Error:[/red] {vault_path} not found.")
        raise typer.Exit(code=100)
    
    with open(vault_path, "r", encoding="utf-8") as f:
        vault = json.load(f)
    accounts = vault.get("accounts", [])

    # Special Unix '-' Handling for stdout
    to_stdout = False
    search_terms = []
    if patterns:
        for p in patterns:
            if p == "-":
                to_stdout = True
            else:
                search_terms.append(p)

    # If no search terms, list ALL
    if not search_terms:
        console.print("[bold blue]All Accounts:[/bold blue]")
        for idx, acc in enumerate(accounts, 1):
            totp = pyotp.TOTP(acc['secret'], digits=acc.get('digits', 6))
            issuer_str = f"[[yellow]{acc['issuer']}[/yellow]] " if acc.get('issuer') else ""
            console.print(f"{idx:2}) [bold green]{totp.now()}[/bold green] - {issuer_str}{acc['name']}")
        raise typer.Exit(code=0)

    # Handle Index Selection (if last term is digit)
    index_selection = None
    if len(search_terms) > 1 and search_terms[-1].isdigit():
        index_selection = int(search_terms.pop())

    # Execute Search
    results = search_accounts(accounts, search_terms)

    if not results:
        console.print(f"[red]No matches for patterns: {search_terms}[/red]")
        raise typer.Exit(code=1)

    # Selection Logic
    if len(results) == 1 and not force_list:
        target = results[0]
    elif index_selection is not None:
        if 1 <= index_selection <= len(results):
            target = results[index_selection - 1]
        else:
            console.print(f"[red]Error: Index {index_selection} out of range.[/red]")
            raise typer.Exit(code=2)
    else:
        # Display List (Ambiguous or forced)
        title = "Matches" if force_list else f"Ambiguous results ({len(results)} matches)"
        console.print(f"[bold yellow]{title}:[/bold yellow]")
        for idx, acc in enumerate(results, 1):
            totp = pyotp.TOTP(acc['secret'], digits=acc.get('digits', 6))
            issuer_str = f"[[yellow]{acc['issuer']}[/yellow]] " if acc.get('issuer') else ""
            console.print(f"{idx:2}) [bold green]{totp.now()}[/bold green] - {issuer_str}{acc['name']}")
        raise typer.Exit(code=2)

    # Execution: Generate and Output
    totp = pyotp.TOTP(target['secret'], digits=target.get('digits', 6))
    otp_code = totp.now()

    if to_stdout:
        # Pure stdout output
        print(otp_code)
    else:
        pyperclip.copy(otp_code)
        issuer_label = f" ({target['issuer']})" if target.get('issuer') else ""
        console.print(f"Copied OTP for [bold cyan]{target['name']}{issuer_label}[/bold cyan]")
    
    raise typer.Exit(code=0)

if __name__ == "__main__":
    app()
