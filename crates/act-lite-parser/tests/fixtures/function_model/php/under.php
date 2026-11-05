<?php
function f($a, $b, $c, $d, $e) {
    $y = 0;
    if ($a && $b) {
        foreach ($c as $i) {
            switch ($i) {
                case 1:
                    try {
                        $y = $d;
                    } catch (Exception $x) {
                        $y = 2;
                    }
                    break;
                default:
                    $y = 3;
            }
        }
    } elseif ($e) {
        $y = 4;
    }
    $y = $e ? 1 : 2;
    if ($y > 0 || $e) {
        $y = 5;
    }
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    $y = $y + 1;
    return $y;
}
