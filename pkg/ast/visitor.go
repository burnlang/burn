package ast

type Visitor interface {
	VisitProgram(program *Program) interface{}
	VisitTypeDefinition(typeDef *TypeDefinition) interface{}
	VisitFunctionDeclaration(funDecl *FunctionDeclaration) interface{}
	VisitVariableDeclaration(varDecl *VariableDeclaration) interface{}
	VisitBlockStatement(blockStmt *BlockStatement) interface{}
	VisitReturnStatement(returnStmt *ReturnStatement) interface{}
	VisitIfStatement(ifStmt *IfStatement) interface{}
	VisitWhileStatement(whileStmt *WhileStatement) interface{}
	VisitForStatement(forStmt *ForStatement) interface{}
	VisitExpressionStatement(exprStmt *ExpressionStatement) interface{}
	VisitBinaryExpression(binaryExpr *BinaryExpression) interface{}
	VisitUnaryExpression(unaryExpr *UnaryExpression) interface{}
	VisitCallExpression(callExpr *CallExpression) interface{}
	VisitGetExpression(getExpr *GetExpression) interface{}
	VisitSetExpression(setExpr *SetExpression) interface{}
	VisitIndexExpression(indexExpr *IndexExpression) interface{}
	VisitSliceExpression(sliceExpr *SliceExpression) interface{}
	VisitArrayLiteralExpression(arrayLiteral *ArrayLiteralExpression) interface{}
	VisitStructLiteralExpression(structLiteral *StructLiteralExpression) interface{}
	VisitClassMethodCallExpression(callExpr *ClassMethodCallExpression) interface{}
	VisitVariableExpression(varExpr *VariableExpression) interface{}
	VisitAssignmentExpression(assignExpr *AssignmentExpression) interface{}
	VisitCompoundAssignmentExpression(compoundExpr *CompoundAssignmentExpression) interface{}
	VisitLiteralExpression(literalExpr *LiteralExpression) interface{}
	VisitGroupingExpression(groupingExpr *GroupingExpression) interface{}
	VisitLambdaExpression(lambdaExpr *LambdaExpression) interface{}
	VisitThisExpression(thisExpr *ThisExpression) interface{}
	VisitNilExpression(nilExpr *NilExpression) interface{}
	VisitCastExpression(castExpr *CastExpression) interface{}
	VisitRangeExpression(rangeExpr *RangeExpression) interface{}
	VisitErrorNode(errorNode *ErrorNode) interface{}
}

func (p *Program) Accept(visitor Visitor) interface{} {
	return visitor.VisitProgram(p)
}

func (t *TypeDefinition) Accept(visitor Visitor) interface{} {
	return visitor.VisitTypeDefinition(t)
}

func (f *FunctionDeclaration) Accept(visitor Visitor) interface{} {
	return visitor.VisitFunctionDeclaration(f)
}

func (v *VariableDeclaration) Accept(visitor Visitor) interface{} {
	return visitor.VisitVariableDeclaration(v)
}

func (b *BlockStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitBlockStatement(b)
}

func (r *ReturnStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitReturnStatement(r)
}

func (i *IfStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitIfStatement(i)
}

func (w *WhileStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitWhileStatement(w)
}

func (f *ForStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitForStatement(f)
}

func (e *ExpressionStatement) Accept(visitor Visitor) interface{} {
	return visitor.VisitExpressionStatement(e)
}

func (b *BinaryExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitBinaryExpression(b)
}

func (u *UnaryExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitUnaryExpression(u)
}

func (c *CallExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitCallExpression(c)
}

func (g *GetExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitGetExpression(g)
}

func (s *SetExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitSetExpression(s)
}

func (i *IndexExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitIndexExpression(i)
}

func (s *SliceExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitSliceExpression(s)
}

func (a *ArrayLiteralExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitArrayLiteralExpression(a)
}

func (s *StructLiteralExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitStructLiteralExpression(s)
}

func (c *ClassMethodCallExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitClassMethodCallExpression(c)
}

func (v *VariableExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitVariableExpression(v)
}

func (a *AssignmentExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitAssignmentExpression(a)
}

func (c *CompoundAssignmentExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitCompoundAssignmentExpression(c)
}

func (l *LiteralExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitLiteralExpression(l)
}

func (g *GroupingExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitGroupingExpression(g)
}

func (l *LambdaExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitLambdaExpression(l)
}

func (t *ThisExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitThisExpression(t)
}

func (n *NilExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitNilExpression(n)
}

func (c *CastExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitCastExpression(c)
}

func (r *RangeExpression) Accept(visitor Visitor) interface{} {
	return visitor.VisitRangeExpression(r)
}

func (e *ErrorNode) Accept(visitor Visitor) interface{} {
	return visitor.VisitErrorNode(e)
}
