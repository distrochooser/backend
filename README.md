# distrochooser-backend

This repository contains an experimental implementation of the distrochooser.de-backend. 

## Flaws

It is an experiment. It's slow and I do not know Rust at all.

## Routes

- [x] /distributions/:lang/
- [x] /distribution/:id/:lang/
- [x] /questions/:lang/
- [x] /i18n/:lang/
- [ ] /newvisitor/:lang/
- [ ] /addresult/:lang/

## Usage

`rusty_distrochooser /path/to/db.conf`

db.conf:
mysql://$username:$passord@$host