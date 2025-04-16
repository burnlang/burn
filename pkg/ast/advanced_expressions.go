package ast

type CompoundAssignmentExpression struct {
	Name     string
	Operator string
	Value    Expression
	Position int
}

func (c *CompoundAssignmentExpression) expressionNode() {}
func (c *CompoundAssignmentExpression) Pos() int {
	return c.Position
}

func (c *CompoundAssignmentExpression) String() string {
	return "CompoundAssignmentExpression: " + c.Name + " " + c.Operator
}

type LiteralExpression struct {
	Value    interface{}
	Type     string
	Raw      string
	Position int
}

func (l *LiteralExpression) expressionNode() {}
func (l *LiteralExpression) Pos() int {
	return l.Position
}

func (l *LiteralExpression) String() string {
	return "LiteralExpression: " + l.Raw
}

type GroupingExpression struct {
	Expression Expression
	Position   int
}

func (g *GroupingExpression) expressionNode() {}
func (g *GroupingExpression) Pos() int {
	return g.Position
}

func (g *GroupingExpression) String() string {
	return "GroupingExpression"
}

type LambdaExpression struct {
	Parameters []Parameter
	ReturnType string
	Body       []Declaration
	Position   int
}

func (l *LambdaExpression) expressionNode() {}
func (l *LambdaExpression) Pos() int {
	return l.Position
}

func (l *LambdaExpression) String() string {
	return "LambdaExpression"
}

type ThisExpression struct {
	Position int
}

func (t *ThisExpression) expressionNode() {}
func (t *ThisExpression) Pos() int {
	return t.Position
}

func (t *ThisExpression) String() string {
	return "ThisExpression"
}

type NilExpression struct {
	Position int
}

func (n *NilExpression) expressionNode() {}
func (n *NilExpression) Pos() int {
	return n.Position
}

func (n *NilExpression) String() string {
	return "NilExpression"
}

type CastExpression struct {
	Expression Expression
	TargetType string
	Position   int
}

func (c *CastExpression) expressionNode() {}
func (c *CastExpression) Pos() int {
	return c.Position
}

func (c *CastExpression) String() string {
	return "CastExpression: to " + c.TargetType
}

type RangeExpression struct {
	Start    Expression
	End      Expression
	Step     Expression
	Position int
}

func (r *RangeExpression) expressionNode() {}
func (r *RangeExpression) Pos() int {
	return r.Position
}

func (r *RangeExpression) String() string {
	return "RangeExpression"
}

type ErrorNode struct {
	Message  string
	Position int
}

func (e *ErrorNode) expressionNode()  {}
func (e *ErrorNode) declarationNode() {}
func (e *ErrorNode) stmtNode()        {}
func (e *ErrorNode) Pos() int {
	return e.Position
}

func (e *ErrorNode) String() string {
	return "ErrorNode: " + e.Message
}
