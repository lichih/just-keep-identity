# Asset: Shell Completions Setup Guide (SOP)
ID: @JKI_ASSET(guide_completions)

This guide provides step-by-step instructions for enabling `jkim` shell completions.

## 1. Bash (Linux / macOS)
Follow these steps to enable completions for the current user.

**Commands:**
```bash
# 1. Create directory
mkdir -p ~/.jki

# 2. Generate completion script
jkim completions bash > ~/.jki/jkim_completion.bash

# 3. Add to configuration
if ! grep -q "jkim_completion.bash" ~/.bashrc 2>/dev/null; then
    echo -e "\n# JKI completions\n[[ -f ~/.jki/jkim_completion.bash ]] && . ~/.jki/jkim_completion.bash" >> ~/.bashrc
fi
```

**Activation:**
```bash
source ~/.bashrc
```

## 2. Zsh (macOS / Linux)
Recommended for users who use Zsh as their primary shell.

**Commands:**
```bash
# 1. Create directory
mkdir -p ~/.jki

# 2. Generate completion script
jkim completions zsh > ~/.jki/_jkim.zsh

# 3. Add to configuration
if ! grep -q "_jkim.zsh" ~/.zshrc 2>/dev/null; then
    echo -e "\n# JKI completions\n[[ -f ~/.jki/_jkim.zsh ]] && source ~/.jki/_jkim.zsh" >> ~/.zshrc
fi
```

**Activation:**
```bash
source ~/.zshrc
```

## 3. Fish
For users who prefer the Fish shell.

**Commands:**
```bash
# 1. Generate directly into fish completion path
mkdir -p ~/.config/fish/completions
jkim completions fish > ~/.config/fish/completions/jkim.fish
```

**Activation:**
Automatically active in new sessions.
