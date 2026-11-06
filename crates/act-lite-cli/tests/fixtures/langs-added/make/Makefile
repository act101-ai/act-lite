CC ?= cc
OBJS := a.o b.o
export PATH
define HELP
text
endef
.PHONY: all clean
all: app
app: $(OBJS)
	$(CC) -o $@ $^
%.o: %.c
	$(CC) -c $<
clean:
	rm -f app
include common.mk
ifeq ($(OS),x)
X = 1
endif
