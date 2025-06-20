FROM denoland/deno:latest

# Install OpenSSL
USER root
RUN apt-get update && apt-get install -y openssl ffmpeg && apt-get clean
# Set working directory
WORKDIR /app

# Copy project files and the script
COPY . /app
COPY generate-certs.sh /app/generate-certs.sh
RUN chmod +x /app/generate-certs.sh

# Expose ports
EXPOSE 8000

# Run the cert generation script, then start the Deno app
CMD ["/bin/sh", "-c", "/app/generate-certs.sh && deno task start"]
