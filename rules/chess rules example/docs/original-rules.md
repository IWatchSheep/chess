# Original program steps

These steps are the program behavior the three definitions below are meant to complete. They are unchanged.

Step 1: When I select a chess piece, the program displays all its available legal moves.

Step 2: Identifying Pieces Without Legal Moves

If I select a chess piece that has no available legal moves, the program displays a message explaining that the selected piece cannot currently make a legal move.

However, chess pieces without available legal moves must still be considered when identifying forming chess strategies in Step 8.

A piece's inability to move legally does not automatically exclude it from contributing to a developing chess strategy.

Step 3: Moving and Confirming a Chess Piece's Move

If I select a chess piece that has available legal moves, I can choose a destination square by entering its two-character coordinate and pressing Enter.

Before the move is finalized, I am asked whether I want to confirm it.

If I enter "Y", the move is finalized, and the chess piece moves to the selected square.

If I enter "N", the move is canceled, and the chess piece remains on its original square.

The program then returns to Step 1, allowing me to select either the same chess piece or a different chess piece.

After I select a piece, the program checks its available legal moves again.

1. If the selected piece has legal moves, the program displays all available legal moves, and I can choose a destination square.
2. If the selected piece has no legal moves, the program follows Step 2 and displays a message explaining that the piece cannot currently make a legal move. I can then return to Step 1 to select another piece.

No changes are made to the chessboard until I confirm a legal move by entering "Y".

Step 4: I want all the pieces on the board to have their available legal moves listed before and after a piece's move to a square is finalized. These lists should be recorded and kept in two separate files: one for White's turn and one for Black's turn.

Step 5: Step 4 creates two separate files: one for White's turns and one for Black's turns. Each file should maintain a record of the previous four turns and the current turn for its respective player. The records should include all available legal moves before and after each move is finalized. No additional files are needed for Step 5 because the two files created in Step 4 will also serve as the history files for Step 5.

Step 6: Calculate and Record Three Full Turns Into the Future for White and Black

I want the chess program to examine three full turns into the future, which means three individual moves by White and three individual moves by Black, for a total search depth of six individual moves.

The program must examine every legal move available at each step, including all possible responses from the opposing player.

From the standard starting position, the number of possible legal move sequences increases as follows:

- Individual move 1 (White): 20 possible sequences.
- Individual move 2 (Black): 400 possible sequences.
- Individual move 3 (White): 8,902 possible sequences.
- Individual move 4 (Black): 197,281 possible sequences.
- Individual move 5 (White): 4,865,609 possible sequences.
- Individual move 6 (Black): 119,060,324 possible sequences.

The total number of search nodes across all six depths is:

20 + 400 + 8,902 + 197,281 + 4,865,609 + 119,060,324 = **124,132,536 search nodes**.

These numbers apply to the standard starting position. As the game progresses, the number of legal moves and possible future sequences will change depending on the current board position.

**Future-Search Files**

I want two separate future-search files maintained: one for White and one for Black.

- The White file should record the three-full-turn future search from positions where White is the next player to move.
- The Black file should record the three-full-turn future search from positions where Black is the next player to move.

Each future search must account for both players' possible moves, not just the moves of the player associated with the file.

The two files should be updated as the game progresses so that their information reflects the current board position and its possible future moves.

The purpose of these files is to provide the future move information needed to identify and evaluate developing chess strategies.

**Search Depth for Lists 1 and 3**

List 1 in Step 8 uses the current board position, the previous four turns, the current turn, and one full turn of future information to identify forming chess strategies.

List 3 in Step 12 uses the same future-search information from Step 6, but begins its strategy analysis one individual move into the future.

For List 3, the program must:

1. Examine every possible legal first move from the current board position.
2. Use the resulting board position as the starting point for evaluating future forming strategies.
3. Continue examining the legal move sequences already generated by Step 6.
4. Use the remaining five individual moves within the existing search to identify how strategies may form and develop.
5. Keep each strategy associated with the future move and board position that produced it.

**List 3 must not restart a new six-individual-move search after advancing one move into the future.**

The original search depth of six individual moves remains the limit for both List 1 and List 3.

List 3 therefore uses the existing search results from Step 6 rather than extending the search to seven or eight individual moves.

The same search results are also used to evaluate the potential captures associated with List 3 when creating List 4 in Step 13.

**Performance and File Management**

The program must preserve the complete three-full-turn search logic.

Performance optimization and file management should be handled separately without changing the required search depth or excluding legal move sequences.

Step 7: I want the history files and future-search files to be managed automatically so they remain organized, cleaned, and up to date as the game progresses. At the beginning of every new game, all four files should be cleared, and the information needed for the new game should be generated.

Step 8: Creating List 1 — Forming Chess Strategies

The four lists begin with List 1, which identifies chess strategies that are currently forming.

To create List 1, the program uses the following information:

1. **Step 4 and Step 5:** The appropriate White or Black history file, containing the previous four turns and the current turn.
2. **Step 6:** The appropriate White or Black future-search file, examining one full turn into the future.
3. **The 20 chess strategies:** The program compares the information from the past four turns, the current turn, and one future turn against the 20 chess strategies to identify which strategies are beginning to form.

The program combines the information from these two files and evaluates it against the 20 chess strategies.

The results are stored in **List 1, called the Forming Strategies List**.

List 1 contains the chess strategies identified as forming based on the player's previous four turns, current turn, and one full turn into the future.

Step 9: The list created in Step 8 (List 1) is sorted according to how fully each chess strategy has developed. The most fully developed strategy appears at the top of the list, while the least developed strategy appears at the bottom.

Step 10: Creating List 2 — Chess Strategy Captures

List 2 identifies the opposing chess pieces that can be captured by the strategies listed in List 1 from Step 9.

For each strategy, List 2 should record:

1. The name of the chess strategy.
2. The chess piece belonging to the player that can make the capture.
3. The opposing chess piece that can be captured.
4. The square occupied by the capturing piece and the square occupied by the opposing piece.
5. The potential cost of losing the capturing piece as a result of the capture.

Each strategy should be listed alongside its possible captures, clearly identifying which piece can capture which opposing piece.

The purpose of List 2 is to identify the potential captures associated with each forming chess strategy in List 1.

Step 11: Sorting List 2 — Chess Capture Values

The list created in Step 10 (List 2) is sorted according to the overall value of each potential chess capture.

The value of each capture is calculated by considering:

1. The value of the opposing chess piece that can be captured.
2. The value of the player's chess piece making the capture.
3. The potential cost of losing the capturing piece as a result of the capture.

Each capture is evaluated based on its potential gain compared to its potential loss.

List 2 is then sorted from the most valuable capture at the top to the least valuable capture at the bottom, taking into account the risk of losing the capturing piece.

Step 12: Creating List 3 — Future Forming Chess Strategies

Using the appropriate White or Black future-search file created in Step 6, the program examines the chessboard one individual move into the future.

At that future position, the program repeats the strategy-identification process described in Step 8, using the 20 chess strategies to identify which strategies are forming.

The results are stored in List 3 instead of List 1.

List 3 is then sorted using the same method described in Step 9, placing the most fully developed strategy at the top and the least developed strategy at the bottom.

The purpose of List 3 is to identify how chess strategies may develop after the next individual move, allowing the program to compare future forming strategies with those currently identified in List 1.

The program must repeat this evaluation for every possible legal next move, identifying the forming strategies associated with each resulting future board position. Each result must remain associated with the move and board position that produced it.

Step 13: Creating and Sorting List 4 — Future Chess Strategy Captures

List 4 repeats the capture-identification and sorting processes described in Steps 10 and 11, but uses the future forming chess strategies from List 3 instead of the current forming strategies from List 1.

For each strategy in List 3, List 4 should record:

1. The name of the future forming chess strategy.
2. The chess piece belonging to the player that can make the capture.
3. The opposing chess piece that can be captured.
4. The square occupied by the capturing piece and the square occupied by the opposing piece.
5. The potential cost of losing the capturing piece as a result of the capture.

Each potential capture is evaluated based on the value of the opposing piece that can be captured and the potential cost of losing the capturing piece.

List 4 is then sorted from the most valuable potential capture at the top to the least valuable potential capture at the bottom.

The purpose of Lists 2 and 4 is to identify and rank the potential captures associated with the chess strategies in Lists 1 and 3.

- **List 2** evaluates and ranks potential captures associated with the current forming strategies in List 1.
- **List 4** evaluates and ranks potential captures associated with the future forming strategies in List 3.
