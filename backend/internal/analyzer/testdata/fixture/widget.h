#ifndef WIDGET_H
#define WIDGET_H

/* Forward declaration: must NOT become a building of its own. */
typedef struct Widget Widget;

struct Palette {
    int fg;
    int bg;
};

typedef struct {
    int x, y;
    int w, h;
} Rect;

enum Mode { MODE_A, MODE_B };

Widget *widget_new(int w, int h);
void widget_free(Widget *self);
#endif
