package lexer

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
	TokenTypeKeyword
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
	TokenModulo
	TokenClass
	TokenTypeVoid
)

type Token struct {
	Type     TokenType
	Value    string
	Line     int
	Col      int
	Position int
}

func GetKeywords() map[string]TokenType {
	return map[string]TokenType{
		"fun":    TokenFun,
		"var":    TokenVar,
		"const":  TokenConst,
		"type":   TokenTypeKeyword,
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
		"class":  TokenClass,
		"void":   TokenTypeVoid,
	}
}
