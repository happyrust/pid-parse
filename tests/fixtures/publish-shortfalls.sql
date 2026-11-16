-- One drawing the database line cannot write in full, as a plant database
-- small enough to read. `database_shortfalls` and everything that reports
-- it are tested on this, because no real database in the repository holds
-- an item whose type has no writer of its own.
--
--   DWG-SHORT-01  the drawing, named SHORT-01
--   EX-1          an Exchanger: no writer of its own, so it is written as
--                 a <PIDItem> placeholder
--   V-1, N-1      a Vessel and a Nozzle: written in full
--   R-1           N-1 -> EX-1, a T_Relationship row with an item at each
--                 end: written as a <Rel> whose definition comes from the
--                 two item types alone
--
-- Every column is TEXT, the way the MDF adapter stages a database.

CREATE TABLE T_Drawing (
    SP_ID TEXT PRIMARY KEY, Name TEXT, DocumentCategory TEXT,
    DocumentType TEXT, Template TEXT, Path TEXT, DateCreated TEXT
);
INSERT INTO T_Drawing VALUES (
    'DWG-SHORT-01', 'SHORT-01', 'P&ID Documents', 'P&ID',
    'A2-W-New.pid', '\01\01\SHORT-01.pid', '2026/11/16 10:00:00'
);

CREATE TABLE T_ModelItem (
    SP_ID TEXT PRIMARY KEY, ItemTypeName TEXT, Description TEXT,
    SP_IsTypical TEXT, IsUnchecked TEXT, ModelItemType TEXT,
    UpdateCount TEXT, ItemStatus TEXT
);
INSERT INTO T_ModelItem (SP_ID, ItemTypeName) VALUES
    ('EX-1', 'Exchanger'),
    ('V-1', 'Vessel'),
    ('N-1', 'Nozzle');

CREATE TABLE T_Representation (
    SP_ID TEXT PRIMARY KEY, SP_ModelItemID TEXT, SP_DrawingID TEXT,
    GraphicOID TEXT, FileName TEXT, RepresentationType TEXT
);
INSERT INTO T_Representation VALUES
    ('REP-EX-1', 'EX-1', 'DWG-SHORT-01', '101',
     '\Equipment\Heat Transfer Equipment\Shell and Tube\Exchanger.sym', '13'),
    ('REP-V-1', 'V-1', 'DWG-SHORT-01', '102',
     '\Equipment\Vessels\Horizontal Drums\Horizontal Drum.sym', '13'),
    ('REP-N-1', 'N-1', 'DWG-SHORT-01', '103',
     '\Equipment Components\Nozzles\Flanged Nozzle.sym', '13');

CREATE TABLE T_Relationship (
    SP_ID TEXT PRIMARY KEY, SP_DrawingID TEXT, SP_Item1ID TEXT,
    SP_Item2ID TEXT, GraphicOID TEXT, Item1Location TEXT,
    Item2Location TEXT, IsBinary TEXT
);
INSERT INTO T_Relationship (SP_ID, SP_DrawingID, SP_Item1ID, SP_Item2ID) VALUES
    ('R-1', 'DWG-SHORT-01', 'N-1', 'EX-1');
