package lexer

import (
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"
)

type TokenType int

const (
	TokenEOF TokenType = iota
	TokenIdentifier
	TokenNumber
	TokenString
	TokenPlus
	TokenMinus
	TokenMultiply
	TokenDivide
	TokenAssign
	TokenEqual
	TokenNotEqual
	TokenLess
	TokenGreater
	TokenLessEqual
	TokenGreaterEqual
	TokenLeftParen
	TokenRightParen
	TokenLeftBrace
	TokenRightBrace
	TokenComma
	TokenSemicolon
	TokenColon
	TokenNot
	TokenAnd
	TokenOr
	TokenFun
	TokenVar
	TokenConst
	TokenDef
	TokenIf
	TokenElse
	TokenReturn
	TokenWhile
	TokenFor
	TokenTrue
	TokenFalse
	TokenTypeInt
	TokenTypeFloat
	TokenTypeString
	TokenTypeBool
	TokenDot
	TokenLeftBracket
	TokenRightBracket
	TokenImport
)

type Token struct {
	Type  TokenType
	Value string
	Line  int
	Col   int
}

type Lexer struct {
	source   string
	pos      int
	line     int
	col      int
	tokens   []Token
	keywords map[string]TokenType
}

func New(source string) *Lexer {
	return &Lexer{
		source: source,
		pos:    0,
		line:   1,
		col:    1,
		tokens: []Token{},
		keywords: map[string]TokenType{
			"fun":    TokenFun,
			"var":    TokenVar,
			"const":  TokenConst,
			"def":    TokenDef,
			"if":     TokenIf,
			"else":   TokenElse,
			"return": TokenReturn,
			"while":  TokenWhile,
			"for":    TokenFor,
			"true":   TokenTrue,
			"false":  TokenFalse,
			"int":    TokenTypeInt,
			"float":  TokenTypeFloat,
			"string": TokenTypeString,
			"bool":   TokenTypeBool,
			"import": TokenImport,
		},
	}
}

func (l *Lexer) Tokenize() ([]Token, error) {
	for l.pos < len(l.source) {
		l.skipWhitespace()
		if l.pos >= len(l.source) {
			break
		}

		r, size := utf8.DecodeRuneInString(l.source[l.pos:])
		switch {
		case r == '/':
			if l.pos+1 < len(l.source) && l.source[l.pos+1] == '/' {
				l.skipLineComment()
				continue
			}
			l.addToken(TokenDivide, "/")
			l.advance(size)
		case unicode.IsLetter(r) || r == '_':
			l.tokenizeIdentifier()
		case unicode.IsDigit(r):
			l.tokenizeNumber()
		case r == '"':
			if err := l.tokenizeString(); err != nil {
				return nil, err
			}
		case r == '+':
			l.addToken(TokenPlus, "+")
			l.advance(size)
		case r == '-':
			l.addToken(TokenMinus, "-")
			l.advance(size)
		case r == '*':
			l.addToken(TokenMultiply, "*")
			l.advance(size)
		case r == '=':
			if l.pos+1 < len(l.source) && l.source[l.pos+1] == '=' {
				l.addToken(TokenEqual, "==")
				l.advance(2)
			} else {
				l.addToken(TokenAssign, "=")
				l.advance(size)
			}
		case r == '(':
			l.addToken(TokenLeftParen, "(")
			l.advance(size)
		case r == ')':
			l.addToken(TokenRightParen, ")")
			l.advance(size)
		case r == '{':
			l.addToken(TokenLeftBrace, "{")
			l.advance(size)
		case r == '}':
			l.addToken(TokenRightBrace, "}")
			l.advance(size)
		case r == '[':
			l.addToken(TokenLeftBracket, "[")
			l.advance(size)
		case r == ']':
			l.addToken(TokenRightBracket, "]")
			l.advance(size)
		case r == ',':
			l.addToken(TokenComma, ",")
			l.advance(size)
		case r == ';':
			l.addToken(TokenSemicolon, ";")
			l.advance(size)
		case r == ':':
			l.addToken(TokenColon, ":")
			l.advance(size)
		case r == '<':
			if l.pos+1 < len(l.source) && l.source[l.pos+1] == '=' {
				l.addToken(TokenLessEqual, "<=")
				l.advance(2)
			} else {
				l.addToken(TokenLess, "<")
				l.advance(size)
			}
		case r == '>':
			if l.pos+1 < len(l.source) && l.source[l.pos+1] == '=' {
				l.addToken(TokenGreaterEqual, ">=")
				l.advance(2)
			} else {
				l.addToken(TokenGreater, ">")
				l.advance(size)
			}
		case r == '!':
			if l.pos+1 < len(l.source) && l.source[l.pos+1] == '=' {
				l.addToken(TokenNotEqual, "!=")
				l.advance(2)
			} else {
				l.addToken(TokenNot, "!")
				l.advance(size)
			}
		case r == '&':
			if l.pos+1 < len(l.source) && l.source[l.pos+1] == '&' {
				l.addToken(TokenAnd, "&&")
				l.advance(2)
			} else {
				return nil, fmt.Errorf("unexpected character '&' at line %d, col %d", l.line, l.col)
			}
		case r == '|':
			if l.pos+1 < len(l.source) && l.source[l.pos+1] == '|' {
				l.addToken(TokenOr, "||")
				l.advance(2)
			} else {
				return nil, fmt.Errorf("unexpected character '|' at line %d, col %d", l.line, l.col)
			}
		case r == '.':
			l.addToken(TokenDot, ".")
			l.advance(size)
		default:
			return nil, fmt.Errorf("unexpected character '%c' at line %d, col %d", r, l.line, l.col)
		}
	}

	l.addToken(TokenEOF, "")
	return l.tokens, nil
}

func (l *Lexer) advance(n int) {
	for i := 0; i < n; i++ {
		if l.pos < len(l.source) {
			if l.source[l.pos] == '\n' {
				l.line++
				l.col = 1
			} else {
				l.col++
			}
			l.pos++
		}
	}
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

	// Integer part
	for l.pos < len(l.source) && unicode.IsDigit(rune(l.source[l.pos])) {
		l.advance(1)
	}

	// Decimal part
	if l.pos < len(l.source) && l.source[l.pos] == '.' {
		l.advance(1) // Consume '.'

		if l.pos < len(l.source) && !unicode.IsDigit(rune(l.source[l.pos])) {
			l.pos-- // If no digits follow the '.', backtrack
			l.col--
		} else {
			// Consume decimal digits
			for l.pos < len(l.source) && unicode.IsDigit(rune(l.source[l.pos])) {
				l.advance(1)
			}
		}
	}

	l.addToken(TokenNumber, l.source[start:l.pos])
}

func (l *Lexer) tokenizeString() error {
	start := l.pos
	l.advance(1) // Skip opening quote

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

func (l *Lexer) Position() int {
	return l.pos
}

func processEscapes(s string) string {
	s = strings.ReplaceAll(s, "\\n", "\n")
	s = strings.ReplaceAll(s, "\\t", "\t")
	s = strings.ReplaceAll(s, "\\r", "\r")
	s = strings.ReplaceAll(s, "\\\"", "\"")
	s = strings.ReplaceAll(s, "\\\\", "\\")
	return s
}

func (l *Lexer) addToken(tokenType TokenType, value string) {
	l.tokens = append(l.tokens, Token{
		Type:  tokenType,
		Value: value,
		Line:  l.line,
		Col:   l.col - len(value),
	})
}
