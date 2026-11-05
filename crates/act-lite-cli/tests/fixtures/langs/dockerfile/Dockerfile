FROM alpine:3.19 AS build
RUN echo hi > /out.txt

FROM scratch
COPY --from=build /out.txt /out.txt
