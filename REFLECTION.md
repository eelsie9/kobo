CA2 Reflection

1. Dangling Else

The dangling else issue can be seen in the if_statement function. Lines 203 to 229 show how the dangling else is fixed. Most importantly, lines 216 to 222 show that the then clause is parsed first, via self.statement()? and then the else keyword is checked. Because the self.statement() function is a recursive function, the inner if statement will have been completely processed by the time the outer if statement checks for an else. This means that the else will be attached to the closest un-matched if statement. This dangling else issue was resolved by using recursion.

In the example 'if (false) if (true) print "a"; else print "b";': the else will be attached to the inner if statement. However, since the outer if statement is false, nothing will be printed. If the else was attached to the outer if statement, nothing would be printed.

2. Assignment Check

The assignment check occurs in the assignment function. Lines 287 to 310 show how the assignment check works. The left-hand side expression is first parsed, via self.or()? and then the parser checks for an equals sign. If there is an assignment operator, the right hand side is parsed recursively. Then, the parser checks if the left-hand side is an Expr::Variable. If it is a variable, the assignment expression is constructed. If not, the error is reported via error_at().

The assignment can't be checked until the left-hand side is parsed. As the parser needs to know what kind of expression it is. For example, variables are valid for assignment, however literals like numbers aren't. This is determined by looking at what type of expression structure (AST) is produced.

In the example of 1 = 2; the parser will report the exact error message in Section 5.1. "Cannot assign to this expression".

3. Error Synchronisation

The error synchronisation function occurs in the synchronise function (lines 537 to 553). This allows the parser to recover from a syntax error. This is important so the parser can detect other errors in the same program.

The function checks if the last token consumed was a semicolon. If so, the function will stop. If not, the next token is checked. The function will stop before any of the following tokens: Fun, Var, If, While, Print, Return. These are all potential new declarations/statements. If none of these are found, the parser will skip over the tokens until it finds a good spot to recover or the end of the input.

In my implementation, the synchronise() function is called after a failed declaration and whilst parsing block. This allows the parser to continue parsing after an error.

4. Git History And Debugging

By looking at my Git history, I was able to find and fix an error. In this commit (7e4e32a - almost done), the return_statement function had the following line of code:

let value = if self.check(TokenType::Semicolon) {

This was incorrect as it was checking if the next token was a semicolon. The return statement should only have an expression if the next token isn't a semicolon.

In my final commit (ccab896 - finishing touches), I changed this to:

let value = if !self.check(TokenType::Semicolon) {

This means that the parser will know when to differentiate between a return statement with an expression or not. By comparing the two commits, I could see how important it is to carefully check the conditions of the tokens. Looking at my Git history helps me track my changes, find any mistakes and confirm that they have been fixed.