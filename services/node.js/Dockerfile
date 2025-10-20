# Use an official Node.js runtime as a parent image
FROM node:24-alpine

# Set the working directory in the container
WORKDIR /usr/src/app

RUN apk add python3 make g++

# Copy package.json to the container
COPY package.json ./

# Install dependencies
RUN npm install

# Copy the application source code to the container
COPY app.js ./

# WebSocket Server port
EXPOSE 9002

# Define environment variable
ENV NODE_ENV=production

# Command to run the application
CMD ["node", "app.js"]