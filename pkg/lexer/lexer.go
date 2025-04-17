package lexer

import (
	"fmt"
	"unicode"
	"unicode/utf8"
)

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
		source:   source,
		pos:      0,
		line:     1,
		col:      1,
		tokens:   []Token{},
		keywords: GetKeywords(),
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
		case r == '%':
			l.addToken(TokenModulo, "%")
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

func (l *Lexer) addToken(tokenType TokenType, value string) {
	l.tokens = append(l.tokens, Token{
		Type:     tokenType,
		Value:    value,
		Line:     l.line,
		Col:      l.col - len(value),
		Position: l.pos,
	})
}

func (l *Lexer) Position() int {
	return l.pos
}
