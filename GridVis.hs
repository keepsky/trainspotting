-- Visualization: 
-- * parse infrastructure file (GraphParser.hs)
-- * check if global up/down can be defined (later: find minimal set of up/down reversals to dag-ify)
-- * group linear parts
-- * solve grid (GridSolver.hs)
-- * ungroup linear parts
--
--

module Main where

import System.Environment (getArgs)

import qualified GraphParser as P
import qualified GridSolver as S

import Data.Map (Map)
import qualified Data.Map as Map

import Control.Monad (forM, forM_,join)
import Data.List (mapAccumL, intercalate)

clamp :: Double -> Double -> Double -> Double
clamp a b x = if x < a then a else (if x > b then b else x)

lerp :: Pt Double -> Pt Double -> Double -> Pt Double
lerp (x0,y0) (x1,y1) p = (x0 + (x1-x0)*p, y0 + (y1-y0)*p)

succPairs x = zip x (tail x)

convertInput :: [P.Statement] -> ([S.Node], [String])
convertInput stmts = ([ conv dat | (P.NodeStmt _ dat) <- stmts], idxToName)
  where
    nameToIdx = Map.fromList (zip idxToName [0..])
    idxToName = [name | (P.NodeStmt name _) <- stmts]
    idx x = nameToIdx Map.! x
    
    conv :: P.NodeData -> S.Node
    conv (P.Start name) = S.startNode (idx name)
    conv (P.End name) = S.endNode (idx name)
    conv (P.Sw P.SwLeft  P.Incoming a (b,c)) = S.inLeftSw  (idx a) ((idx b),(idx c))
    conv (P.Sw P.SwRight P.Incoming a (b,c)) = S.inRightSw (idx a) ((idx b),(idx c))
    conv (P.Sw P.SwLeft  P.Outgoing a (b,c)) = S.outLeftSw (idx a) ((idx b),(idx c))
    conv (P.Sw P.SwRight P.Outgoing a (b,c)) = S.outRightSw (idx a) ((idx b),(idx c))

reverseNames :: [String] -> Int -> String
reverseNames = (!!)

solve :: [S.Node] -> IO [S.Graphics]
solve x = go 2
  where
    go dim = do
         -- putStrLn $ "trying dim " ++ (show dim)
         sol <- S.draw x (dim,dim `div` 3)
         case sol of
           Just x -> reduceY (dim,dim `div` 3) x
           Nothing -> if dim > 100 then error "No solution" else go (dim+1)
    reduceY (w,h) sol = do
         -- putStrLn $ "reducing height " ++ (show (w,h))
         sol2 <- S.draw x (w,h-1)
         case sol2 of
           Just sol2 -> reduceY (w,h-1) sol2
           Nothing -> reduceX (w,h) sol
    reduceX (w,h) sol = do
         -- putStrLn $ "reducing width " ++ (show (w,h))
         sol2 <- S.draw x (w-1,h)
         case sol2 of
           Just sol2 -> reduceX (w-1,h) sol2
           Nothing -> return sol

type Pt x = (x,x)
type Line x = (Pt x, Pt x)
type Edge = (String,String)

edgeCoords :: [Pt Int] -> [(Edge,Double)] -> [(Edge, [Line Double])]
edgeCoords screen edges = (snd (mapAccumL f 0.0 nonZeroEdges))
  where
    edgelength = sum [ x | (_,x) <- edges ]
    nonZeroEdges = [ x | x@(_,l) <- edges, l > 0.0 ]
    e2s x = (x / edgelength) * (fromIntegral ((length screen)-1))

    f :: Double -> (Edge,Double) -> (Double, (Edge, [Line Double]))
    f l (e, dl) = (l + dl, (e, lines (e2s l) (e2s (l+dl))))

    lines :: Double -> Double -> [Line Double]
    lines s0 s1 = [ (a,b) | (start, (p1i,p2i)) <- zip [0..] (succPairs screen)
                          , let p1 = (fromIntegral $ fst p1i, fromIntegral $ snd p1i)
                          , let p2 = (fromIntegral $ fst p2i, fromIntegral $ snd p2i)
                          , start <= s1 && start+1 >= s0
                          , let a = lerp p1 p2 (clamp 0.0 1.0 (s0 - start))
                          , let b = lerp p2 p1 (clamp 0.0 1.0 ((start+1) - s1)) ]

mkLevel :: [Pt Int] -> Int
mkLevel = (* (-1)) . sum . (map snd)

jsonNodeCoords :: [(Edge, [Line Double])] -> String
jsonNodeCoords xs = "{" ++ (intercalate ",\n" (fmap obj xs)) ++ "}"
  where
    obj :: (Edge, [Line Double]) -> String
    obj ((n1,n2),ls) = "\"" ++ n1 ++ "-" ++ n2 ++"\": [ " ++ l ++ " ]" 
      where l = intercalate "," (fmap lin ls)
            lin ((x0,y0),(x1,y1)) = "[[" ++ (show x0) ++ "," ++ (show x1) ++ "],[" ++ (show y0) ++ "," ++ (show y1) ++ "]]"

javascriptOutput :: String -> String
javascriptOutput x = "var edges = " ++ x ++ ";"

main = do
  args <- getArgs
  let filename = args !! 0
  (Right graph) <- P.parseFile filename
  let (problem,names) = convertInput graph
  sol <- solve problem
  let edges = S.collectEdges sol
  let edgesA = [((reverseNames names a, reverseNames names b, (mkLevel c)), c) | ((a,b),c) <- edges ]
  let edgesB = [ ((a,b,l),c) | (P.EdgeStmt (a,b) l c) <- graph ]
  let edgesAB = join [ edgeCoords screen dist | ((_,screen),(_,dist)) <- zip edgesA edgesB ]
  putStrLn $ javascriptOutput (jsonNodeCoords edgesAB)

