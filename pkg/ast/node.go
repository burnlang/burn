package ast

type Node interface {
	Pos() int
}

type Expression interface {
	Node
	expressionNode()
}

type Declaration interface {
	Node
	declarationNode()
}

type Statement interface {
	Declaration
	stmtNode()
}

type Program struct {
	Declarations []Declaration
	Position     int
}

func (p *Program) Pos() int {
	return p.Position
}

func (p *Program) String() string {
	return "Program"
}
