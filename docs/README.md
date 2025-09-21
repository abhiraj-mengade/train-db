# TrainDB GitHub Pages

This directory contains the static website for TrainDB that's served via GitHub Pages.

## Setup GitHub Pages

To enable GitHub Pages for this repository:

1. Go to your repository settings on GitHub
2. Scroll down to "Pages" section
3. Under "Source", select "Deploy from a branch"
4. Choose "main" branch and "/docs" folder
5. Click "Save"

Your site will be available at: `https://yourusername.github.io/train-db/`

## Files

- `index.html` - Main website showcasing TrainDB features
- `demo_bluetooth.sh` - Downloadable demo script
- `README.md` - This file

## Customization

Update the following in `index.html`:
- Replace `yourusername` with your actual GitHub username in all URLs
- Update meta tags with your repository URL
- Customize colors, content, and branding as needed

## Local Development

To preview locally, simply open `index.html` in your browser or use a local server:

```bash
# Using Python
python -m http.server 8000

# Using Node.js
npx serve .

# Then visit http://localhost:8000
```
