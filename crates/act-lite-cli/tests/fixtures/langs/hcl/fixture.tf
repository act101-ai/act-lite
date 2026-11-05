variable "region" {
  default = "us-east-1"
}

resource "aws_s3_bucket" "logs" {
  bucket = var.region
}
