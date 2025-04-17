package ast

type BlockStatement struct {
	Statements []Declaration
	Position   int
}

func (b *BlockStatement) declarationNode() {}
func (b *BlockStatement) stmtNode()        {}
func (b *BlockStatement) Pos() int {
	return b.Position
}

func (b *BlockStatement) String() string {
	return "BlockStatement"
}

type ReturnStatement struct {
	Value    Expression
	Position int
}

func (r *ReturnStatement) declarationNode() {}
func (r *ReturnStatement) stmtNode()        {}
func (r *ReturnStatement) Pos() int {
	return r.Position
}

func (r *ReturnStatement) String() string {
	return "ReturnStatement"
}

type IfStatement struct {
	Condition  Expression
	ThenBranch []Declaration
	ElseBranch []Declaration
	Position   int
}

func (i *IfStatement) declarationNode() {}
func (i *IfStatement) stmtNode()        {}
func (i *IfStatement) Pos() int {
	return i.Position
}

func (i *IfStatement) String() string {
	return "IfStatement"
}

type WhileStatement struct {
	Condition Expression
	Body      []Declaration
	Position  int
}

func (w *WhileStatement) declarationNode() {}
func (w *WhileStatement) stmtNode()        {}
func (w *WhileStatement) Pos() int {
	return w.Position
}

func (w *WhileStatement) String() string {
	return "WhileStatement"
}

type ForStatement struct {
	Initializer Declaration
	Condition   Expression
	Increment   Expression
	Body        []Declaration
	Position    int
}

func (f *ForStatement) declarationNode() {}
func (f *ForStatement) stmtNode()        {}
func (f *ForStatement) Pos() int {
	return f.Position
}

func (f *ForStatement) String() string {
	return "ForStatement"
}

type ExpressionStatement struct {
	Expression Expression
	Position   int
}

func (e *ExpressionStatement) declarationNode() {}
func (e *ExpressionStatement) stmtNode()        {}
func (e *ExpressionStatement) Pos() int {
	return e.Position
}

func (e *ExpressionStatement) String() string {
	return "ExpressionStatement"
}
