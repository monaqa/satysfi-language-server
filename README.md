# [WIP] SATySFi Language Server

This repository is work-in-progress yet.

## Features

|Kind             |Function                                                   |Done|
|:----------------|:----------------------------------------------------------|:--:|
|`codeAction`     |Add the definition of an undefined command under the cursor|    |
|`completion`     |Complete a command name                                    |    |
|`completion`     |Complete a field name in a record                          |    |
|`completion`     |Complete a local function/variable name                    |    |
|`completion`     |Complete a primitive                                       |✅  |
|`completion`     |Complete a public function in a module                     |    |
|`diagnostics`    |Linter (warning)                                           |    |
|`diagnostics`    |Syntax error (Recoverable)                                 |    |
|`diagnostics`    |Syntax error (Unrecoverable)                               |✅  |
|`diagnostics`    |Type error                                                 |    |
|`format`         |Code formatting                                            |    |
|`gotoDeclaration`|Go to the type declaration of a command in a module        |    |
|`gotoDeclaration`|Go to the type declaration of a public function in a module|    |
|`gotoDefinition` |Go to the definiton of a command                           |    |
|`gotoDefinition` |Go to the definiton of a local function/variable           |    |
|`gotoDefinition` |Go to the definiton of a public function in a module       |    |
|`hover`          |Hover on a command in a module                             |    |
|`hover`          |Hover on a primitive                                       |✅  |
|`hover`          |Hover on a public function in a module                     |    |
|`rename`         |Rename a variable name                                     |    |
|`typeHint`       |Type hints after a command                                 |    |
