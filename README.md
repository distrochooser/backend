# distrochooser-backend

This repository contains an experimental implementation of the distrochooser.de-backend. 

## Flaws

It is an experiment. It's slow and I do not know Rust at all.

## Routes

- [x] /distributions/:lang/
- [x] /distribution/:id/:lang/
- [x] /questions/:lang/
- [x] /i18n/:lang/
- [x] /newvisitor/
- [x] /get/:lang/ (combines /distributions /questions /i18n and /newvisitor)
- [ ] /addresult/:lang/

## Build

`cargo run /path/to/db.conf`

## Usage

`rusty_distrochooser /path/to/db.conf`

db.conf:
mysql://$username:$password@$host