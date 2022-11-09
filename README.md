---
title: Read Me!
---

# Personal Inventory(pinv)
### A small-scale SQLite inventory manager written in rust for the average hobbyist

## Usage
### Base usage
```
pinv [mode] [options]

MODES:
    add
    add-catagory
    grab
    search
    modify
    delete
```
### Add mode usage
```
pinv add [options]

OPTIONS(manditory):
    -k or --key
    -c or --catagory
    -q or --quantity
    -l or --location
    -f or --fields
```
### Add-Catagory mode
```
pinv add-catagory [catagory-id] [options]

OPTIONS(manditory):
    -f or --fields
```

## To-Do:
- [ ] Program database I/O
    - [x] Allow creation of catagories
    - [x] Allow creation of entries
    - [x] Allow retrieval of specific entries by key
    - [x] Allow searching through entries
    - [ ] Allow modifying entries by key
    - [ ] Allow deleting entries by key
    - [ ] Store database in user's data folder
- [ ] Program CLI
    - [ ] Devise usage format
    - [ ] Generate label image from template
- [ ] Program TUI