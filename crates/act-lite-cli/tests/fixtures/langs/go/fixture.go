package main

type Point struct {
	X int
	Y int
}

func Add(a int, b int) int { return a + b }

func main() {
	Add(1, 2)
}
