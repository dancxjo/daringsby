FROM denoland/deno:latest

# Install OpenSSL for certificate generation
USER root
RUN apt-get update && apt-get install -y openssl && apt-get clean

# Set working directory inside the container
WORKDIR /app

# Copy the local project files to the container
COPY . /app

# Copy a script to generate certs
COPY generate-certs.sh /app/generate-certs.sh
RUN chmod +x /app/generate-certs.sh

# Expose any ports your Deno app may need
EXPOSE 8000
EXPOSE 80

# Run cert generation script before starting the app
CMD ["/bin/sh", "-c", "/app/generate-certs.sh && deno task start"]
