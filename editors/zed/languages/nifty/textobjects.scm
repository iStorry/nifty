(function_declaration
  body: (block "{" (_)* @function.inside "}")) @function.around

(view_declaration
  body: (block "{" (_)* @function.inside "}")) @function.around

(struct_declaration
  body: (field_declaration_list "{" (_)* @class.inside "}")) @class.around

(comment)+ @comment.around
