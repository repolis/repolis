package main

import (
	"fmt"
	"os"

	geo "example.com/app/geom"
	"example.com/app/store"
	"example.com/app/util"
)

type App struct {
	s     *store.Store
	count int
	name  string
}

type Runner interface {
	Run() error
	Stop()
}

func (a *App) Run() error {
	a.s.Put("k", 1)
	_ = geo.Area(2.0)
	_ = util.Clamp(a.count, 0, 10)
	fmt.Println(os.Args)
	return nil
}

func (a App) Stop() {}

// A plain function. Go says it belongs to no type.
func New() *App {
	return &App{s: store.NewStore()}
}

var Version = "1"
