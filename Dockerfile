# Base image for Deno
FROM denoland/deno:latest

# Set working directory inside the container
WORKDIR /app

# Copy the local project files to the container
COPY . /app

# Expose any ports your Deno app may need (e.g., 8000)
EXPOSE 8000
EXPOSE 80

# Set the entry point to run "deno task dev" on container start
CMD ["deno", "task", "start"]