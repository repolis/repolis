#include "widget.h"
#include <stdlib.h>

struct Widget {
    Rect bounds;
    struct Palette colors;
    int visible;
    char *label;
};

static int counter;

Widget *widget_new(int w, int h) {
    Widget *self = malloc(sizeof(Widget));
    self->bounds = rect_make(0, 0, w, h);
    counter++;
    return self;
}

void widget_free(Widget *self) { free(self); }

/* Name says widget, first parameter says Palette: the ambiguous case. */
int widget_recolor(struct Palette *p, int fg) {
    p->fg = fg;
    return 0;
}

Rect rect_make(int x, int y, int w, int h) {
    Rect r;
    r.x = x; r.y = y; r.w = w; r.h = h;
    return r;
}

/* Belongs to no type at all: must become part of the file's module. */
int clamp(int v, int lo, int hi) {
    if (v < lo) return lo;
    if (v > hi) return hi;
    return v;
}

int main(int argc, char **argv) {
    Widget *w = widget_new(10, 20);
    widget_free(w);
    return clamp(argc, 0, 1);
}
