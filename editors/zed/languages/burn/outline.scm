(function_declaration
  (modifier)* @context
  "fun" @context
  name: (_) @name) @item

(definition
  "def" @context
  (definition_kind) @context
  name: (_) @name) @item

(object_method
  object: (_) @context
  name: (_) @name) @item

(field_declaration
  name: (_) @name) @item

(enum_variant) @name @item
