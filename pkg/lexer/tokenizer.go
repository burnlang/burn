package lexer

import (
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"
)

func (l *Lexer) tokenizeIdentifier() {
	start := l.pos

	for l.pos < len(l.source) {
		r, size := utf8.DecodeRuneInString(l.source[l.pos:])
		if unicode.IsLetter(r) || unicode.IsDigit(r) || r == '_' {
			l.advance(size)
		} else {
			break
		}
	}

	value := l.source[start:l.pos]
	if tokenType, isKeyword := l.keywords[value]; isKeyword {
		l.addToken(tokenType, value)
	} else {
		l.addToken(TokenIdentifier, value)
	}
}

func (l *Lexer) tokenizeNumber() {
	start := l.pos

	for l.pos < len(l.source) && unicode.IsDigit(rune(l.source[l.pos])) {
		l.advance(1)
	}

	if l.pos < len(l.source) && l.source[l.pos] == '.' {
		l.advance(1)

		if l.pos < len(l.source) && !unicode.IsDigit(rune(l.source[l.pos])) {

			l.pos--
			l.col--
		} else {

			for l.pos < len(l.source) && unicode.IsDigit(rune(l.source[l.pos])) {
				l.advance(1)
			}
		}
	}

	l.addToken(TokenNumber, l.source[start:l.pos])
}

func (l *Lexer) tokenizeString() error {
	start := l.pos
	l.advance(1)

	for l.pos < len(l.source) && l.source[l.pos] != '"' {

		if l.source[l.pos] == '\\' && l.pos+1 < len(l.source) {
			l.advance(2)
		} else {
			l.advance(1)
		}
	}

	if l.pos >= len(l.source) {
		return fmt.Errorf("unterminated string at line %d", l.line)
	}

	value := processEscapes(l.source[start+1 : l.pos])
	l.addToken(TokenString, value)
	l.advance(1)
	return nil
}

func processEscapes(s string) string {
	s = strings.ReplaceAll(s, "\\n", "\n")
	s = strings.ReplaceAll(s, "\\t", "\t")
	s = strings.ReplaceAll(s, "\\r", "\r")
	s = strings.ReplaceAll(s, "\\\"", "\"")
	s = strings.ReplaceAll(s, "\\\\", "\\")
	return s
}

func (l *Lexer) skipWhitespace() {
	for l.pos < len(l.source) {
		r, size := utf8.DecodeRuneInString(l.source[l.pos:])
		if unicode.IsSpace(r) {
			l.advance(size)
		} else {
			break
		}
	}
}

func (l *Lexer) skipLineComment() {
	l.advance(2)

	for l.pos < len(l.source) && l.source[l.pos] != '\n' {
		l.advance(1)
	}
}
