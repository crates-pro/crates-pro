FROM almalinux:8.10-20250307 AS sensleak-worker

COPY ./sensleak_worker ./sensleak_worker
COPY ./scan ./scan
COPY ./.env ./.env

ENV RUST_BACKTRACE=1
CMD ["./sensleak_worker"]
