# Numerical language reference

NexusLab supports a growing MATLAB-style numerical language. It is useful for
array calculations, small linear systems, polynomial fitting, and signal
analysis. It does not yet run general Octave or MATLAB programs unchanged.

## Arrays and operators

```matlab
t = 0:0.01:1;          % Row vector with 101 samples
A = [1 2 3;4 5 6];     % Two rows, three columns
column = A(:,2);
corner = A(1:2,2:end);
flat = A(:);            % [1;4;2;5;3;6], in column-major order
A(:,2) = [20;50];
A(end) = 60;
```

Indices start at one. A subscript can be an integer, a range, an index array,
or `:`. `end` resolves within the corresponding subscript. Two subscripts
select the Cartesian product of rows and columns. A linear selection uses
column-major order, although the engine stores arrays internally by row.
`A(:)` always returns a column vector. Vector selections preserve the source
vector's orientation when both source and index are vectors.

Indexed assignment replaces existing elements and accepts matching data or a
scalar. Invalid indices or dimensions leave that array unchanged. Assignment
does not grow arrays, delete elements, or accept logical masks.

| Expression | Meaning |
| --- | --- |
| `A + B`, `A - B` | Matching arrays or scalar broadcasting |
| `A * B` | Matrix multiplication; scalars scale an array |
| `A .* B`, `A ./ B`, `A .^ B` | Element-wise multiplication, division, and power |
| `A'` | Real transpose; conjugate transpose for a complex vector |
| `A^n` | Integer matrix power, including zero and negative exponents |
| `A \ b`, `solve(A,b)` | LU solve for square systems; SVD pseudoinverse for rectangular systems |
| `A / B` | Right matrix division; division by a scalar scales each element |

Arrays support real `f64` values. FFT results and nonreal eigenvalues use
complex vectors; general complex matrix algebra is not implemented. Fractional
matrix powers, nonconjugating `.'`, implicit row/column broadcasting, and
operator overloading are not supported.

Whitespace matters in matrix literals: `[1 -2]` contains two elements, while
`[1 - 2]` contains the result of subtraction. Use commas when an expression
would otherwise be ambiguous. Block matrix concatenation is not supported.

## Matrix functions

| Function | Result |
| --- | --- |
| `zeros(n)`, `ones(n)`, `randn(n)` | An `n × n` array |
| `zeros(r,c)`, `ones(r,c)`, `randn(r,c)` | An `r × c` array |
| `zeros([r c])`, `ones(size(A))`, `randn(size(A))` | Array with the supplied two-dimensional shape |
| `eye(n)`, `eye(r,c)` | Square or rectangular identity array |
| `size(A)`, `size(A,dimension)` | Dimensions or the length of one dimension |
| `reshape(A,r,c)` | New shape preserving column-major element order |
| `det(A)` | Determinant of a square array |
| `inv(A)` | Inverse of a nonsingular square array |
| `eig(A)` | Unsorted eigenvalues as a column vector, including complex pairs |
| `pinv(A)`, `pinv(A,tolerance)` | Moore–Penrose pseudoinverse using SVD |
| `polyfit(x,y,degree)` | Least-squares coefficients in descending powers |
| `polyval(coefficients,x)` | Polynomial evaluated at each coordinate |

`eig` provides eigenvalues only. Eigenvectors, generalized eigenproblems, and
multi-output calls are not supported. The default `pinv` cutoff is
`max(rows,columns) × epsilon × largest_singular_value`; an explicit tolerance
is an absolute singular-value cutoff. Prefer a solve over an explicit inverse
when calculating a solution. Singular square solves report an error; use
`pinv(A)*b` explicitly for a minimum-norm solution.

Decompositions are limited to 256 rows and columns, and iterative SVD/Schur
decompositions have bounded iteration counts. Matrix products and powers have
work limits. Polynomial degree is limited to 16; center and scale large sample
coordinates before fitting. Array generation and selection are limited to one
million elements. These limits keep interactive calculations manageable.

Statistics currently summarize the complete array, rather than reducing each
matrix column as Octave does. `norm(A)` is the Frobenius norm for matrices.
`stats(A)` reports mean, median, sample standard deviation, RMS, and peak-to-peak
range. NexusLab filtering uses `filter(signal,taps)` and `sosfilt(signal,sos)`;
these signatures differ from Octave's filtering functions.

## Learning in the application

Open **Tutorial** or **Help → NexusAcademy**. The contents tree groups nine
lessons into Getting Started, Vectors & Matrices, Plotting Data, and DSP &
Filters. Each syntax-highlighted code block has **Try it** and a copy button.
Try it opens the command window, records the code in command history, and
executes it in the current workspace. A plot opens the Figures view.

Examples are self-contained and use `academy_` variable names. Re-running an
example replaces its lesson variables. Select **Reference** for the command
list and supported-language limits. The Tutorial panel can be detached,
closed, or reopened through View, like the other workbench panels.

## Compatibility boundaries and migration

M-file control flow (`for`, `while`, `if`), user-defined M-file functions,
multi-output assignment, sparse arrays, cells, structures, and Octave packages
are not available. Rhai supplies bounded loops and conditionals with its own
syntax. Reading a `.m` file does not imply complete MATLAB compatibility.

Version 0.4 corrects several earlier behaviors. `A^n` now means matrix power;
use `A.^n` for element-wise power. `A/B` now means right division; use `A./B`
for element-wise division. Single-size constructors now produce square arrays;
replace old `zeros(n)` vector calls with `zeros(1,n)` (similarly for `ones` and
`randn`). Existing real and complex-row `.nxw` files remain readable. Files
containing the new complex-column values require version 0.4 or later.

Indexing and operator conventions follow the
[GNU Octave indexing reference](https://docs.octave.org/latest/Index-Expressions.html)
and [arithmetic reference](https://docs.octave.org/latest/Arithmetic-Ops.html).
The [matrix function reference](https://docs.octave.org/latest/Basic-Matrix-Functions.html)
describes the broader Octave functionality beyond this supported subset.
