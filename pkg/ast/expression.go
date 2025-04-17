package ast

type BinaryExpression struct {
	Left     Expression
	Operator string
	Right    Expression
	Position int
}

func (b *BinaryExpression) expressionNode() {}
func (b *BinaryExpression) Pos() int {
	return b.Position
}

func (b *BinaryExpression) String() string {
	return "BinaryExpression: " + b.Operator
}

type UnaryExpression struct {
	Operator string
	Right    Expression
	Position int
}

func (u *UnaryExpression) expressionNode() {}
func (u *UnaryExpression) Pos() int {
	return u.Position
}

func (u *UnaryExpression) String() string {
	return "UnaryExpression: " + u.Operator
}

type CallExpression struct {
	Callee    Expression
	Arguments []Expression
	Position  int
}

func (c *CallExpression) expressionNode() {}
func (c *CallExpression) Pos() int {
	return c.Position
}

func (c *CallExpression) String() string {
	return "CallExpression"
}

type GetExpression struct {
	Object   Expression
	Name     string
	Position int
}

func (g *GetExpression) expressionNode() {}
func (g *GetExpression) Pos() int {
	return g.Position
}

func (g *GetExpression) String() string {
	return "GetExpression: ." + g.Name
}

type SetExpression struct {
	Object   Expression
	Name     string
	Value    Expression
	Position int
}

func (s *SetExpression) expressionNode() {}
func (s *SetExpression) Pos() int {
	return s.Position
}

func (s *SetExpression) String() string {
	return "SetExpression: ." + s.Name
}

type IndexExpression struct {
	Array    Expression
	Index    Expression
	Position int
}

func (i *IndexExpression) expressionNode() {}
func (i *IndexExpression) Pos() int {
	return i.Position
}

func (i *IndexExpression) String() string {
	return "IndexExpression"
}

type SliceExpression struct {
	Array    Expression
	Start    Expression
	End      Expression
	Position int
}

func (s *SliceExpression) expressionNode() {}
func (s *SliceExpression) Pos() int {
	return s.Position
}

func (s *SliceExpression) String() string {
	return "SliceExpression"
}

type ArrayLiteralExpression struct {
	Elements []Expression
	Position int
}

func (a *ArrayLiteralExpression) expressionNode() {}
func (a *ArrayLiteralExpression) Pos() int {
	return a.Position
}

func (a *ArrayLiteralExpression) String() string {
	return "ArrayLiteralExpression"
}

type StructLiteralExpression struct {
	Type     string
	Fields   map[string]Expression
	Position int
}

func (s *StructLiteralExpression) expressionNode() {}
func (s *StructLiteralExpression) Pos() int {
	return s.Position
}

func (s *StructLiteralExpression) String() string {
	return "StructLiteralExpression"
}

type ClassMethodCallExpression struct {
	ClassName  string
	MethodName string
	Arguments  []Expression
	IsStatic   bool
	Position   int
}

func (c *ClassMethodCallExpression) expressionNode() {}
func (c *ClassMethodCallExpression) Pos() int {
	return c.Position
}

func (c *ClassMethodCallExpression) String() string {
	methodType := "instance"
	if c.IsStatic {
		methodType = "static"
	}
	return "ClassMethodCallExpression: " + c.ClassName + "." + c.MethodName + " (" + methodType + ")"
}

type VariableExpression struct {
	Name     string
	Position int
}

func (v *VariableExpression) expressionNode() {}
func (v *VariableExpression) Pos() int {
	return v.Position
}

func (v *VariableExpression) String() string {
	return "VariableExpression: " + v.Name
}

type AssignmentExpression struct {
	Name     string
	Value    Expression
	Position int
}

func (a *AssignmentExpression) expressionNode() {}
func (a *AssignmentExpression) Pos() int {
	return a.Position
}

func (a *AssignmentExpression) String() string {
	return "AssignmentExpression: " + a.Name
}
